"""Material conversion orchestration and page checkpoint lifecycle."""

from __future__ import annotations

import json
import os
import time
import uuid
import zipfile
from pathlib import Path
from threading import Event, Lock
from typing import Callable, Dict, Iterable, List, Mapping, Optional, Sequence, Tuple

from .common import (
    CONTRACT_VERSION,
    ExtractionContext,
    IdentityAllocator,
    ResourceStore,
    ensure_within,
    is_uuid,
    require_uuid,
    sha256_path,
)
from .corrections import replay_corrections
from .errors import EngineError, safe_error
from .manifest import (
    assemble_manifest,
    read_checkpoint,
    validate_manifest,
    write_checkpoint,
    write_manifest,
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
    unprocessed = [area for area in coverage.get("unprocessed", []) if area.get("page_id") in (None, page_id)]
    failed = [area for area in coverage.get("failed", []) if area.get("page_id") in (None, page_id)]
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
        payload, source_path, staging_root, parse_version_id, requested_pages, ids, parent_parse_version_id = self._validate_payload(payload)
        task_id = require_uuid(task_id or payload.get("task_id"), "task_id")
        source_hash = sha256_path(source_path)
        supplied_hash = payload.get("source_hash")
        if supplied_hash is not None and supplied_hash != source_hash:
            raise EngineError("STALE_INPUT", "source hash changed before conversion", scope="asset", details={"expected_hash": str(supplied_hash), "actual_hash": source_hash})
        existing_manifest = _read_json(staging_root / "manifest.json") if resume else None
        checkpoint = read_checkpoint(staging_root) if resume else None
        if existing_manifest is not None:
            if existing_manifest.get("source_hash") != source_hash or existing_manifest.get("parse_version_id") != parse_version_id:
                raise EngineError("IDEMPOTENCY_MISMATCH", "resume staging does not match source or parse version", scope="task")
        if checkpoint is not None and checkpoint.get("task_id") not in (None, task_id):
            raise EngineError("IDEMPOTENCY_MISMATCH", "checkpoint belongs to another task", scope="task")
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
        if existing_manifest is not None:
            ctx.resources.load_existing(existing_manifest.get("resources", []))
        renderer = PageRenderer(ctx)
        cancel_event = Event()
        with self._lock:
            self._cancel_events[task_id] = cancel_event
            if task_id in self._pending_cancels:
                cancel_event.set()
                self._pending_cancels.discard(task_id)
        drafts: List[PageDraft] = []
        if existing_manifest is not None:
            for index in requested_pages:
                previous = _draft_from_manifest(existing_manifest, index)
                if previous is not None:
                    drafts.append(previous)
        completed = set(checkpoint.get("completed_pages", []) if checkpoint else [])
        retry_pages = set(payload.get("retry_pages", []) if isinstance(payload.get("retry_pages", []), list) else [])
        pages_to_process = [index for index in requested_pages if index not in completed or index in retry_pages]
        # A fresh request always processes all requested pages.  A resume with
        # no checkpoint is intentionally equivalent to a fresh request.
        if not resume or checkpoint is None:
            pages_to_process = requested_pages
        budget_ms = payload.get("budget_ms")
        deadline = time.monotonic() + (float(budget_ms) / 1000.0) if isinstance(budget_ms, (int, float)) and budget_ms > 0 else None
        source_kind = _source_kind(source_path)
        corrections = payload.get("corrections", [])
        if not isinstance(corrections, list):
            raise EngineError("INVALID_ARGUMENT", "corrections must be a list", scope="command")

        def current_draft(index: int) -> Optional[PageDraft]:
            return next((draft for draft in drafts if draft.index == index), None)

        def emit_page(draft: PageDraft) -> None:
            if on_page is None:
                return
            on_page(
                {
                    "parse_version_id": parse_version_id,
                    "source_page_index": draft.index,
                    "page": draft.page,
                    "regions": draft.regions,
                    "objects": draft.objects,
                    "resources": [resource for resource in ctx.resources.records() if any(owner in {draft.page_id} | {obj["object_id"] for obj in draft.objects} for owner in resource.get("owner_refs", []))],
                    "search_units": draft.search_units,
                    "search_coverage": {"processed": draft.processed_for_search, "unprocessed": draft.coverage_unprocessed, "failed": draft.coverage_failed},
                }
            )

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
                            page_drafts = extract_pdf(ctx, [page_index], renderer)
                        elif source_kind == "pptx":
                            page_drafts = extract_pptx(ctx, [page_index], renderer)
                        elif source_kind == "text":
                            page_drafts = extract_text(ctx, [page_index], renderer)
                        else:
                            page_drafts = extract_image(ctx, [page_index], renderer)
                        draft = page_drafts[0]
                    except EngineError as exc:
                        # A single page error is isolated so other pages and
                        # recording tasks are not made to wait or fail.
                        draft = _failed_draft(ctx, page_index, exc.as_safe_error())
                    except Exception as exc:  # pragma: no cover - malformed third-party input guard
                        draft = _failed_draft(ctx, page_index, safe_error("CORRUPT_ASSET", "page parser failed", retryable=True, scope="page", details={"exception": type(exc).__name__}))
                previous = current_draft(page_index)
                drafts = [item for item in drafts if item.index != page_index]
                drafts.append(draft)
                if draft.page.get("status") != "failed":
                    completed.add(page_index)
                checkpoint_value = {
                    "contract_version": CONTRACT_VERSION,
                    "task_id": task_id,
                    "parse_version_id": parse_version_id,
                    "source_hash": source_hash,
                    "profile": payload["profile"],
                    "requested_pages": requested_pages,
                    "completed_pages": sorted(completed),
                    "failed_pages": sorted(item.index for item in drafts if item.page.get("status") == "failed"),
                    "state": "running",
                }
                try:
                    partial_manifest = assemble_manifest(ctx, drafts, requested_pages, renderer_diagnostics=renderer.diagnostics)
                except EngineError:
                    # Keep the page checkpoint even if a partially accumulated
                    # package cannot yet satisfy its final closure.
                    partial_manifest = None
                if partial_manifest is not None:
                    write_manifest(staging_root, partial_manifest)
                write_checkpoint(staging_root, checkpoint_value)
                if on_checkpoint is not None:
                    on_checkpoint(checkpoint_value)
                emit_page(draft)
                if cancel_event.is_set():
                    raise EngineError("CANCELLED", "conversion was cancelled", retryable=True, scope="task")

            replayed, conflicts = replay_corrections(ctx, drafts, corrections)
            manifest = assemble_manifest(
                ctx,
                drafts,
                requested_pages,
                corrections_applied=replayed,
                unresolved_conflicts=conflicts,
                renderer_diagnostics=renderer.diagnostics,
            )
            manifest_path = write_manifest(staging_root, manifest)
            completed_checkpoint = {
                "contract_version": CONTRACT_VERSION,
                "task_id": task_id,
                "parse_version_id": parse_version_id,
                "source_hash": source_hash,
                "profile": payload["profile"],
                "requested_pages": requested_pages,
                "completed_pages": sorted(completed),
                "failed_pages": sorted(item.index for item in drafts if item.page.get("status") == "failed"),
                "state": "completed",
            }
            write_checkpoint(staging_root, completed_checkpoint)
            if on_checkpoint is not None:
                on_checkpoint(completed_checkpoint)
            return {"manifest": manifest, "manifest_path": str(manifest_path), "checkpoint": completed_checkpoint}
        finally:
            renderer.close()
            with self._lock:
                self._cancel_events.pop(task_id, None)
