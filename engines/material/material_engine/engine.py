"""Material conversion orchestration and page checkpoint lifecycle."""

from __future__ import annotations

import json
import time
import zipfile
from copy import deepcopy
from pathlib import Path
from threading import Event, Lock
from typing import Callable, Dict, Iterable, List, Mapping, Optional, Tuple

from .common import (
    CONTRACT_VERSION,
    ExtractionContext,
    IdentityAllocator,
    ResourceStore,
    require_uuid,
    sha256_path,
)
from .corrections import replay_corrections
from .errors import EngineError, safe_error
from .manifest import (
    assemble_manifest,
    read_checkpoint,
    read_snapshot,
    validate_manifest,
    validate_snapshot_header,
    write_checkpoint,
    write_manifest,
    write_snapshot,
    snapshot_path,
)
from .models import PageDraft
from .pdf import _page_order, _parse_objects, extract_pdf
from .pptx import _slide_paths, extract_pptx
from .render import PageRenderer
from .simple import extract_image, extract_text


ENGINE_BUILD = "material-engine/0.1.0"
ENGINE_CONTRACT_VERSION = CONTRACT_VERSION


PageCallback = Callable[[dict], None]
CheckpointCallback = Callable[[dict], None]


def _read_json(path: Path) -> Optional[dict]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, ValueError):
        return None
    return value if isinstance(value, dict) else None


def _read_manifest_file(path: Path) -> dict:
    value = _read_json(path)
    if value is None:
        raise EngineError("CORRUPT_ASSET", "manifest handle is not valid JSON", scope="asset")
    return value


def _source_kind(path: Path) -> str:
    suffix = path.suffix.lower()
    try:
        with path.open("rb") as handle:
            header = handle.read(5)
    except OSError:
        header = b""
    if suffix == ".pdf" or header == b"%PDF-":
        return "pdf"
    if suffix in (".pptx", ".pptm"):
        return "pptx"
    if suffix in (".txt", ".md", ".markdown"):
        return "text"
    if suffix in (".png", ".jpg", ".jpeg", ".gif", ".webp"):
        return "image"
    try:
        with zipfile.ZipFile(str(path), "r") as archive:
            if any(name.startswith("ppt/slides/") for name in archive.namelist()):
                return "pptx"
    except (OSError, zipfile.BadZipFile):
        pass
    return "unsupported"


def _page_count(path: Path, kind: str) -> int:
    if kind == "pdf":
        objects = _parse_objects(path.read_bytes())
        return len(_page_order(objects))
    if kind == "pptx":
        with zipfile.ZipFile(str(path), "r") as archive:
            return len(_slide_paths(archive))
    return 1 if kind in ("text", "image") else 0


def _draft_from_manifest(manifest: dict, page_index: int) -> Optional[PageDraft]:
    page = next((value for value in manifest.get("pages", []) if value.get("source_page_index") == page_index), None)
    if page is None:
        return None
    page_id = page["page_id"]
    page_regions = [region for region in manifest.get("regions", []) if region.get("page_id") == page_id]
    region_ids = {region["region_id"] for region in page_regions}
    page_objects = [obj for obj in manifest.get("objects", []) if region_ids.intersection(obj.get("region_ids", []))]
    page_units = []
    for unit in manifest.get("search_units", []):
        fallback = unit.get("anchor", {}).get("fallback", "")
        if "page:{};".format(page_id) in fallback:
            page_units.append(unit)
    coverage = manifest.get("search_coverage", {})
    # ``page_id=None`` entries are cumulative snapshot-level placeholders for
    # pages that had not been published yet. They must not be copied into
    # every resumed page draft or their count would grow on each resume.
    unprocessed = [area for area in coverage.get("unprocessed", []) if area.get("page_id") == page_id]
    failed = [area for area in coverage.get("failed", []) if area.get("page_id") == page_id]
    return PageDraft(
        index=page_index,
        page=page,
        regions=page_regions,
        objects=page_objects,
        search_units=page_units,
        processed_for_search=page_id in set(coverage.get("processed_pages", [])),
        coverage_unprocessed=unprocessed,
        coverage_failed=failed,
    )


def _failed_draft(ctx: ExtractionContext, page_index: int, error: dict, reason: str = "page_failed") -> PageDraft:
    page_id = ctx.ids.id("page", str(page_index))
    root_region_id = ctx.ids.id("region", "{}:root".format(page_index))
    page = {
        "page_id": page_id,
        "revision_id": ctx.ids.id("page_revision", str(page_index)),
        "source_page_index": page_index,
        "display_label": str(page_index + 1),
        "width": 612,
        "height": 792,
        "unit": "pt",
        "rotation": 0,
        "source_transform": [1, 0, 0, 1, 0, 0],
        "preview_resource_id": None,
        "reading_order": [],
        "region_ids": [root_region_id],
        "status": "failed",
        "errors": [error],
    }
    return PageDraft(
        index=page_index,
        page=page,
        regions=[{"region_id": root_region_id, "page_id": page_id, "bbox": [0, 0, 1, 1], "object_ids": [], "semantic_role": "unknown", "parent_region_id": None}],
        coverage_failed=[{"page_id": page_id, "region_id": root_region_id, "reason": reason}],
    )


class MaterialEngine:
    """Reusable conversion service used by the JSONL sidecar protocol."""

    def __init__(self) -> None:
        self._cancel_events: Dict[str, Event] = {}
        self._pending_cancels = set()
        self._lock = Lock()

    @staticmethod
    def capabilities() -> dict:
        return {
            "supported_contract_majors": [2],
            "supported_material_majors": [2],
            "visualization_protocol_majors": [],
            "templates": {},
            "search_projection_versions": [1],
            "focaus_result_semantics": [],
        }

    def cancel(self, task_id: str) -> None:
        with self._lock:
            event = self._cancel_events.get(task_id)
            if event is not None:
                event.set()
            elif len(self._pending_cancels) < 128:
                # The JSONL reader can receive cancel immediately after a
                # convert request, before the worker has registered its event.
                # Keep a bounded marker so that early cancellation is not
                # silently lost or allowed to grow without limit.
                self._pending_cancels.add(task_id)

    @staticmethod
    def _publication_id(
        ids: IdentityAllocator,
        staging_root: Path,
        task_id: str,
        sequence: int,
        *,
        final_id: Optional[str],
        existing_ids: Iterable[str],
    ) -> str:
        existing = set(str(value) for value in existing_ids)
        if final_id is not None:
            if final_id in existing or snapshot_path(staging_root, final_id).is_file():
                raise EngineError(
                    "IDEMPOTENCY_MISMATCH",
                    "final parse_version_id is already published",
                    scope="task",
                    details={"parse_version_id": str(final_id)},
                )
            return final_id
        return ids.unique_id(
            "parse_publication",
            "{}:{}".format(task_id, sequence),
            existing,
        )

    @staticmethod
    def _snapshot_payload(
        manifest: dict,
        path: Path,
        staging_root: Path,
        *,
        page: Optional[PageDraft] = None,
        publication_sequence: Optional[int] = None,
    ) -> dict:
        """Build a detached page result so later correction/reparse cannot mutate it."""
        payload = {
            "parse_version_id": manifest["parse_version_id"],
            "parent_parse_version_id": manifest.get("parent_parse_version_id"),
            "snapshot_parse_version_id": manifest["parse_version_id"],
            "manifest_path": str(path),
            "package_root": str(staging_root),
            "manifest": deepcopy(manifest),
        }
        if publication_sequence is not None:
            payload["publication_sequence"] = publication_sequence
        if page is not None:
            payload.update(
                {
                    "source_page_index": page.index,
                    "page": deepcopy(page.page),
                    "regions": deepcopy(page.regions),
                    "objects": deepcopy(page.objects),
                    "search_units": deepcopy(page.search_units),
                    "search_coverage": {
                        "processed": page.processed_for_search,
                        "unprocessed": deepcopy(page.coverage_unprocessed),
                        "failed": deepcopy(page.coverage_failed),
                    },
                }
            )
        return payload

    def _validate_payload(self, payload: Mapping[str, object]) -> Tuple[dict, Path, Path, str, List[int], IdentityAllocator, Optional[str]]:
        if not isinstance(payload, Mapping):
            raise EngineError("INVALID_ARGUMENT", "conversion payload must be an object", scope="command")
        material_id = require_uuid(payload.get("material_id"), "material_id")
        source_version_id = require_uuid(payload.get("source_version_id"), "source_version_id")
        identity_pool = payload.get("identity_pool") or {}
        if not isinstance(identity_pool, Mapping):
            raise EngineError("INVALID_ARGUMENT", "identity_pool must be an object", scope="command")
        ids = IdentityAllocator(material_id, source_version_id, identity_pool)  # type: ignore[arg-type]
        parse_version_raw = payload.get("parse_version_id")
        parse_version_id = require_uuid(parse_version_raw, "parse_version_id") if parse_version_raw else ids.id("parse_version", "current")
        source_raw = payload.get("source_handle")
        staging_raw = payload.get("staging_handle")
        if not isinstance(source_raw, str) or "://" in source_raw:
            raise EngineError("INVALID_ARGUMENT", "source_handle must be a local managed file path", scope="asset")
        if not isinstance(staging_raw, str):
            raise EngineError("INVALID_ARGUMENT", "staging_handle must be a local directory", scope="asset")
        source_path = Path(source_raw).expanduser().resolve()
        staging_root = Path(staging_raw).expanduser().resolve()
        if not source_path.is_file():
            raise EngineError("SOURCE_UNAVAILABLE", "source_handle does not point to a readable file", retryable=True, scope="asset")
        try:
            staging_root.mkdir(parents=True, exist_ok=True)
        except OSError as exc:
            raise EngineError("PERMISSION_DENIED", "staging directory cannot be created", scope="asset", details={"exception": type(exc).__name__})
        profile = payload.get("profile")
        if profile not in ("light", "deep"):
            raise EngineError("INVALID_ARGUMENT", "profile must be light or deep", scope="command")
        pages_raw = payload.get("pages", [])
        if not isinstance(pages_raw, list) or any(not isinstance(page, int) or page < 0 for page in pages_raw):
            raise EngineError("INVALID_ARGUMENT", "pages must be a list of non-negative integers", scope="command")
        if len(set(pages_raw)) != len(pages_raw):
            raise EngineError("INVALID_ARGUMENT", "pages must not contain duplicates", scope="command")
        source_kind = _source_kind(source_path)
        if source_kind == "unsupported":
            raise EngineError("UNSUPPORTED_CAPABILITY", "material source type is not supported by this engine", scope="asset", details={"suffix": source_path.suffix.lower()})
        count = _page_count(source_path, source_kind)
        if count <= 0:
            raise EngineError("CORRUPT_ASSET", "material source contains no readable pages", scope="asset")
        requested_pages = list(pages_raw) if pages_raw else list(range(count))
        if any(page >= count for page in requested_pages):
            raise EngineError("INVALID_ARGUMENT", "requested page is outside source range", scope="page", details={"page_count": str(count)})
        parent_raw = payload.get("parent_parse_version_id")
        parent = require_uuid(parent_raw, "parent_parse_version_id") if parent_raw else None
        return dict(payload), source_path, staging_root, parse_version_id, requested_pages, ids, parent

    def convert(
        self,
        payload: Mapping[str, object],
        *,
        task_id: Optional[str] = None,
        resume: bool = False,
        on_page: Optional[PageCallback] = None,
        on_checkpoint: Optional[CheckpointCallback] = None,
    ) -> dict:
        """Convert with immutable publication snapshots.

        Each page checkpoint publishes a cumulative immutable snapshot under a new
        parse ID.  The requested parse ID is reserved for the final snapshot,
        whose parent is the last page publication (or the verified base
        snapshot when no page was published in this attempt).
        """
        payload, source_path, staging_root, parse_version_id, requested_pages, ids, parent_parse_version_id = self._validate_payload(payload)
        task_id = require_uuid(task_id or payload.get("task_id"), "task_id")
        source_hash = sha256_path(source_path)
        supplied_hash = payload.get("source_hash")
        if supplied_hash is not None and supplied_hash != source_hash:
            raise EngineError(
                "STALE_INPUT",
                "source hash changed before conversion",
                scope="asset",
                details={"expected_hash": str(supplied_hash), "actual_hash": source_hash},
            )

        head_path = staging_root / "manifest.json"
        existing_head = _read_json(head_path) if head_path.is_file() else None
        checkpoint = read_checkpoint(staging_root) if resume else None
        if not resume and checkpoint is not None:
            raise EngineError(
                "IDEMPOTENCY_MISMATCH",
                "staging directory already has a task checkpoint; resume it or choose a new staging directory",
                scope="task",
            )
        if resume and checkpoint is not None and checkpoint.get("parse_version_id") != parse_version_id:
            raise EngineError(
                "IDEMPOTENCY_MISMATCH",
                "resume parse_version_id does not match the original task target",
                scope="task",
                details={"expected": str(checkpoint.get("parse_version_id")), "actual": str(parse_version_id)},
            )
        if existing_head is not None:
            if existing_head.get("material_id") != payload["material_id"] or existing_head.get("source_version_id") != payload["source_version_id"]:
                raise EngineError("CONFLICT", "staging head belongs to another material source", scope="asset")
            if existing_head.get("source_hash") != source_hash:
                raise EngineError("STALE_INPUT", "staging head was created from another source hash", scope="asset")

        target_snapshot = read_snapshot(staging_root, parse_version_id)
        if target_snapshot is not None:
            if resume and checkpoint and checkpoint.get("state") == "completed":
                validate_snapshot_header(target_snapshot, parse_version_id=parse_version_id, material_id=payload["material_id"])
                validate_manifest(target_snapshot, staging_root)
                return {
                    "manifest": target_snapshot,
                    "manifest_path": str(snapshot_path(staging_root, parse_version_id)),
                    "head_manifest_path": str(head_path),
                    "package_root": str(staging_root),
                    "checkpoint": checkpoint,
                    "published_parse_versions": checkpoint.get("published_parse_versions", [parse_version_id]),
                }
            raise EngineError(
                "IDEMPOTENCY_MISMATCH",
                "parse_version_id is already published; a new parse must use a new ID",
                scope="task",
                details={"parse_version_id": parse_version_id},
            )

        externally_verified_parent_ids = set()
        if parent_parse_version_id is not None:
            parent_snapshot = read_snapshot(staging_root, parent_parse_version_id)
            if parent_snapshot is not None:
                validate_snapshot_header(
                    parent_snapshot,
                    parse_version_id=parent_parse_version_id,
                    material_id=payload["material_id"],
                )
                validate_manifest(parent_snapshot, staging_root)
            else:
                parent_handle = payload.get("parent_manifest_handle")
                if not isinstance(parent_handle, str) or "://" in parent_handle:
                    raise EngineError(
                        "SOURCE_UNAVAILABLE",
                        "parent_parse_version_id is not a published local snapshot",
                        retryable=True,
                        scope="asset",
                        details={"parent_parse_version_id": str(parent_parse_version_id)},
                    )
                parent_path = Path(parent_handle).expanduser().resolve()
                if not parent_path.is_file():
                    raise EngineError("SOURCE_UNAVAILABLE", "parent manifest handle is unavailable", retryable=True, scope="asset")
                parent_snapshot = _read_manifest_file(parent_path)
                validate_snapshot_header(
                    parent_snapshot,
                    parse_version_id=parent_parse_version_id,
                    material_id=payload["material_id"],
                )
                parent_root = parent_path.parent.parent if parent_path.parent.name == "snapshots" else parent_path.parent
                validate_manifest(parent_snapshot, parent_root)
                externally_verified_parent_ids.add(parent_parse_version_id)

        if resume:
            if existing_head is None or checkpoint is None:
                raise EngineError("SOURCE_UNAVAILABLE", "resume requires a local head manifest and checkpoint", retryable=True, scope="task")
            head_id = checkpoint.get("head_parse_version_id") or existing_head.get("parse_version_id")
            if not isinstance(head_id, str) or read_snapshot(staging_root, head_id) is None:
                raise EngineError("SOURCE_UNAVAILABLE", "resume head snapshot is missing", retryable=True, scope="task")
            published_parse_versions = list(checkpoint.get("published_parse_versions", [head_id]))
            if head_id not in published_parse_versions:
                published_parse_versions.append(head_id)
            existing_manifest = existing_head
            completed = set(checkpoint.get("completed_pages", []))
            publication_sequence = int(checkpoint.get("next_publication_sequence", len(published_parse_versions)))
        else:
            head_id = None
            published_parse_versions = []
            existing_manifest = None
            completed = set()
            publication_sequence = 0

        ctx = ExtractionContext(
            source_path=source_path,
            source_hash=source_hash,
            material_id=payload["material_id"],
            source_version_id=payload["source_version_id"],
            parse_version_id=parse_version_id,
            parent_parse_version_id=parent_parse_version_id,
            profile=payload["profile"],
            staging_root=staging_root,
            ids=ids,
            resources=ResourceStore(staging_root, ids),
        )
        # A new parse can share immutable resource bytes with the current
        # staging head.  Loading verifies every old byte before reuse.
        if existing_head is not None:
            ctx.resources.load_existing(existing_head.get("resources", []))

        renderer = PageRenderer(ctx)
        cancel_event = Event()
        with self._lock:
            self._cancel_events[task_id] = cancel_event
            if task_id in self._pending_cancels:
                cancel_event.set()
                self._pending_cancels.discard(task_id)

        drafts: List[PageDraft] = []
        if resume:
            for index in requested_pages:
                previous = _draft_from_manifest(existing_manifest, index)
                if previous is not None:
                    drafts.append(previous)
        retry_pages = set(payload.get("retry_pages", []) if isinstance(payload.get("retry_pages", []), list) else [])
        pages_to_process = [index for index in requested_pages if index not in completed or index in retry_pages]
        if not resume:
            pages_to_process = requested_pages
        budget_ms = payload.get("budget_ms")
        deadline = time.monotonic() + (float(budget_ms) / 1000.0) if isinstance(budget_ms, (int, float)) and budget_ms > 0 else None
        source_kind = _source_kind(source_path)
        corrections = payload.get("corrections", [])
        if not isinstance(corrections, list):
            raise EngineError("INVALID_ARGUMENT", "corrections must be a list", scope="command")

        def publish_snapshot(
            publication_id: str,
            publication_parent_id: Optional[str],
            *,
            corrections_applied: Optional[List[dict]] = None,
            unresolved_conflicts: Optional[List[str]] = None,
            publication_kind: str,
            sequence: int,
        ) -> Tuple[dict, Path]:
            manifest = assemble_manifest(
                ctx,
                drafts,
                requested_pages,
                corrections_applied=corrections_applied,
                unresolved_conflicts=unresolved_conflicts,
                renderer_diagnostics=renderer.diagnostics,
                publication_parse_version_id=publication_id,
                publication_parent_parse_version_id=publication_parent_id,
                expected_page_count=len(requested_pages),
                provenance_extra={
                    "publication_kind": publication_kind,
                    "publication_sequence": sequence,
                    "publication_parent_verified": publication_parent_id is None or publication_parent_id in published_parse_versions or publication_parent_id in externally_verified_parent_ids,
                    "base_parent_parse_version_id": ctx.parent_parse_version_id,
                    "publication_chain": list(published_parse_versions) + [publication_id],
                    "immutable_snapshot_relative_path": "snapshots/{}.json".format(publication_id),
                },
            )
            path = write_snapshot(
                staging_root,
                manifest,
                externally_verified_parent_ids=externally_verified_parent_ids,
            )
            # This is a replaceable recovery/head pointer.  The immutable
            # snapshot above is the only parse-version identity.
            write_manifest(staging_root, manifest)
            return manifest, path

        def emit_page(manifest: dict, path: Path, page: PageDraft, sequence: int) -> None:
            if on_page is None:
                return
            on_page(self._snapshot_payload(manifest, path, staging_root, page=page, publication_sequence=sequence))

        try:
            for page_index in pages_to_process:
                if cancel_event.is_set():
                    raise EngineError("CANCELLED", "conversion was cancelled", retryable=True, scope="task")
                if deadline is not None and time.monotonic() >= deadline:
                    error = safe_error("DEADLINE_EXCEEDED", "conversion budget reached before this page", retryable=True, scope="page", details={"page_index": str(page_index)})
                    draft = _failed_draft(ctx, page_index, error, reason="budget_exhausted")
                else:
                    try:
                        if source_kind == "pdf":
                            draft = extract_pdf(ctx, [page_index], renderer)[0]
                        elif source_kind == "pptx":
                            draft = extract_pptx(ctx, [page_index], renderer)[0]
                        elif source_kind == "text":
                            draft = extract_text(ctx, [page_index], renderer)[0]
                        else:
                            draft = extract_image(ctx, [page_index], renderer)[0]
                    except EngineError as exc:
                        draft = _failed_draft(ctx, page_index, exc.as_safe_error())
                    except Exception as exc:  # pragma: no cover - malformed asset boundary
                        draft = _failed_draft(
                            ctx,
                            page_index,
                            safe_error("CORRUPT_ASSET", "page parser failed", retryable=True, scope="page", details={"exception": type(exc).__name__}),
                        )
                drafts = [item for item in drafts if item.index != page_index]
                drafts.append(draft)
                if draft.page.get("status") != "failed":
                    completed.add(page_index)

                sequence = publication_sequence
                publication_sequence += 1
                publication_id = self._publication_id(
                    ids,
                    staging_root,
                    task_id,
                    sequence,
                    final_id=None,
                    existing_ids=published_parse_versions + ([parse_version_id] if parse_version_id else []),
                )
                publication_parent_id = head_id or parent_parse_version_id
                manifest, snapshot = publish_snapshot(
                    publication_id,
                    publication_parent_id,
                    publication_kind="page_checkpoint",
                    sequence=sequence,
                )
                published_parse_versions.append(publication_id)
                head_id = publication_id
                checkpoint_value = {
                    "contract_version": CONTRACT_VERSION,
                    "task_id": task_id,
                    "parse_version_id": parse_version_id,
                    "head_parse_version_id": head_id,
                    "published_parse_versions": list(published_parse_versions),
                    "next_publication_sequence": publication_sequence,
                    "source_hash": source_hash,
                    "profile": payload["profile"],
                    "requested_pages": requested_pages,
                    "completed_pages": sorted(completed),
                    "failed_pages": sorted(item.index for item in drafts if item.page.get("status") == "failed"),
                    "state": "running",
                }
                write_checkpoint(staging_root, checkpoint_value)
                if on_checkpoint is not None:
                    on_checkpoint(deepcopy(checkpoint_value))
                emit_page(manifest, snapshot, draft, sequence)
                if cancel_event.is_set():
                    raise EngineError("CANCELLED", "conversion was cancelled", retryable=True, scope="task")

            replayed, conflicts = replay_corrections(
                ctx,
                drafts,
                corrections,
                accepted_parse_version_ids=published_parse_versions,
            )
            final_parent_id = head_id or parent_parse_version_id
            final_manifest, final_snapshot = publish_snapshot(
                parse_version_id,
                final_parent_id,
                corrections_applied=replayed,
                unresolved_conflicts=conflicts,
                publication_kind="final",
                sequence=publication_sequence,
            )
            published_parse_versions.append(parse_version_id)
            completed_checkpoint = {
                "contract_version": CONTRACT_VERSION,
                "task_id": task_id,
                "parse_version_id": parse_version_id,
                "head_parse_version_id": parse_version_id,
                "published_parse_versions": list(published_parse_versions),
                "next_publication_sequence": publication_sequence + 1,
                "source_hash": source_hash,
                "profile": payload["profile"],
                "requested_pages": requested_pages,
                "completed_pages": sorted(completed),
                "failed_pages": sorted(item.index for item in drafts if item.page.get("status") == "failed"),
                "state": "completed",
            }
            write_checkpoint(staging_root, completed_checkpoint)
            if on_checkpoint is not None:
                on_checkpoint(deepcopy(completed_checkpoint))
            return {
                "manifest": final_manifest,
                "manifest_path": str(final_snapshot),
                "head_manifest_path": str(head_path),
                "package_root": str(staging_root),
                "checkpoint": completed_checkpoint,
                "published_parse_versions": list(published_parse_versions),
            }
        finally:
            renderer.close()
            with self._lock:
                self._cancel_events.pop(task_id, None)
