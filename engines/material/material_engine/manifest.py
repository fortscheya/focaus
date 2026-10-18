"""Material manifest assembly, closure checks and staging writes."""

from __future__ import annotations

import json
import os
from copy import deepcopy
from pathlib import Path
from typing import Iterable, List, Optional, Sequence

from .common import (
    EXTRACTION_VERSION,
    MATERIAL_FORMAT,
    MATERIAL_FORMAT_VERSION,
    SEARCH_PROFILE,
    ExtractionContext,
    ensure_within,
    is_uuid,
    safe_relative_path,
    sha256_bytes,
    text_hash,
)
from .errors import EngineError
from .models import PageDraft


def _write_json(path: Path, value: dict) -> None:
    temporary = path.with_name(path.name + ".tmp")
    temporary.write_text(json.dumps(value, ensure_ascii=False, sort_keys=True, indent=2) + "\n", encoding="utf-8")
    os.replace(str(temporary), str(path))


def _rebind_material_selectors(value, parse_version_id: str):
    """Copy a manifest value and bind nested material selectors to its snapshot.

    Extractors build page and search anchors before a publication ID exists, so
    their selectors initially carry ``ctx.parse_version_id``.  A page
    checkpoint has its own immutable publication ID and must expose that ID at
    every nested ``MaterialSelector`` as well.  The recursive copy keeps the
    drafts reusable for later checkpoints and deliberately changes only
    material selectors; correction records retain the parse version they were
    submitted against.
    """
    if isinstance(value, dict):
        rebound = {key: _rebind_material_selectors(item, parse_version_id) for key, item in value.items()}
        if value.get("kind") == "material_region" and "parse_version_id" in value:
            rebound["parse_version_id"] = parse_version_id
        return rebound
    if isinstance(value, list):
        return [_rebind_material_selectors(item, parse_version_id) for item in value]
    return value


def _coverage(ctx: ExtractionContext, drafts: Sequence[PageDraft], expected_page_count: Optional[int] = None) -> dict:
    requested_pages = [draft.page_id for draft in drafts]
    processed_pages = [draft.page_id for draft in drafts if draft.processed_for_search]
    unprocessed = [area for draft in drafts for area in draft.coverage_unprocessed]
    failed = [area for draft in drafts for area in draft.coverage_failed]
    if expected_page_count is not None and len(drafts) < expected_page_count:
        unprocessed.extend(
            {
                "page_id": None,
                "region_id": None,
                "reason": "page_not_published_yet",
            }
            for _ in range(expected_page_count - len(drafts))
        )
    if not drafts or (not processed_pages and (unprocessed or failed)):
        state = "unavailable"
    elif unprocessed or failed or len(processed_pages) != len(requested_pages):
        state = "partial"
    else:
        state = "complete"
    return {
        "requested_pages": requested_pages,
        "processed_pages": processed_pages,
        "unprocessed": unprocessed,
        "failed": failed,
        "state": state,
    }


def assemble_manifest(
    ctx: ExtractionContext,
    drafts: Iterable[PageDraft],
    requested_pages: Sequence[int],
    *,
    corrections_applied: Optional[List[dict]] = None,
    unresolved_conflicts: Optional[List[str]] = None,
    renderer_diagnostics: Optional[Sequence[str]] = None,
    publication_parse_version_id: Optional[str] = None,
    publication_parent_parse_version_id: Optional[str] = None,
    expected_page_count: Optional[int] = None,
    provenance_extra: Optional[dict] = None,
) -> dict:
    draft_list = sorted(list(drafts), key=lambda draft: draft.index)
    coverage = _coverage(ctx, draft_list, expected_page_count)
    parse_version_id = publication_parse_version_id or ctx.parse_version_id
    parent_parse_version_id = publication_parent_parse_version_id if publication_parent_parse_version_id is not None else ctx.parent_parse_version_id
    pages = [_rebind_material_selectors(draft.page, parse_version_id) for draft in draft_list]
    regions = [_rebind_material_selectors(region, parse_version_id) for draft in draft_list for region in draft.regions]
    objects = [_rebind_material_selectors(obj, parse_version_id) for draft in draft_list for obj in draft.objects]
    search_units = [_rebind_material_selectors(unit, parse_version_id) for draft in draft_list for unit in draft.search_units]
    statuses = [page["status"] for page in pages]
    if not pages or all(status == "failed" for status in statuses):
        completion = "failed"
    elif coverage["state"] != "complete" or any(status in ("failed", "partial", "unsupported") for status in statuses):
        completion = "partial"
    else:
        completion = "complete"
    if ctx.profile == "light" and pages and all(status in ("basic_ready", "complete") for status in statuses) and coverage["state"] == "complete":
        completion = "complete"
    provenance = {
        "format": "pdf_or_pptx_native_first",
        "extractor_version": EXTRACTION_VERSION,
        "identity_resolution": ctx.ids.provenance(),
        "profile": ctx.profile,
        "search_profile": SEARCH_PROFILE,
        "rendering": {
            "diagnostics": list(renderer_diagnostics or []),
            "status": "renderer_or_fallback",
        },
        "warnings": list(ctx.warnings),
        "formula_evidence": list(ctx.formula_evidence),
        "source_kind": ctx.source_path.suffix.lower().lstrip("."),
        "source_is_immutable_input": True,
    }
    if provenance_extra:
        provenance.update(deepcopy(provenance_extra))
    provenance = _rebind_material_selectors(provenance, parse_version_id)
    manifest = {
        "format": MATERIAL_FORMAT,
        "format_version": MATERIAL_FORMAT_VERSION,
        "material_id": ctx.material_id,
        "source_version_id": ctx.source_version_id,
        "parse_version_id": parse_version_id,
        "parent_parse_version_id": parent_parse_version_id,
        "source_hash": ctx.source_hash,
        "engine_build": "material-engine/0.1.0",
        "parse_profile": ctx.profile,
        "requested_pages": [int(value) for value in requested_pages],
        "pages": pages,
        "regions": regions,
        "objects": objects,
        "resources": ctx.resources.records(),
        "corrections_applied": list(corrections_applied or []),
        "unresolved_conflicts": list(unresolved_conflicts or []),
        "provenance": provenance,
        "completion": completion,
        "search_units": search_units,
        "search_coverage": coverage,
    }
    validate_manifest(manifest, ctx.staging_root)
    return manifest


def _all_ids(manifest: dict) -> List[str]:
    values = []
    for key, id_key in (("pages", "page_id"), ("regions", "region_id"), ("objects", "object_id"), ("resources", "resource_id")):
        values.extend(item[id_key] for item in manifest.get(key, []))
    return values


def _invalid_result(message: str, *, scope: str = "page", details: Optional[dict] = None) -> None:
    raise EngineError("INVALID_RESULT", message, scope=scope, details=details)


def _validate_material_selector(
    selector: dict,
    *,
    manifest: dict,
    page_ids: set,
    region_by_id: dict,
    object_ids: set,
    owner_entity_id: Optional[str] = None,
) -> None:
    kind = selector.get("kind")
    if kind == "material_region":
        if (
            selector.get("material_id") != manifest["material_id"]
            or selector.get("source_version_id") != manifest["source_version_id"]
            or selector.get("parse_version_id") != manifest["parse_version_id"]
        ):
            _invalid_result("material selector escaped the published source snapshot")
        page_id = selector.get("page_id")
        region_id = selector.get("region_id")
        region = region_by_id.get(region_id)
        if page_id not in page_ids or region is None or region.get("page_id") != page_id:
            _invalid_result("material selector page or region is outside the package")
    elif kind == "object":
        object_id = selector.get("object_id")
        if object_id not in object_ids:
            _invalid_result("object selector owner is outside the package")
        if owner_entity_id is not None and object_id != owner_entity_id:
            _invalid_result("object selector owner does not match its anchor")
    elif kind not in ("audio", "text", "whole"):
        _invalid_result("material output contains an unknown selector kind")


def _validate_source_ref(
    source: dict,
    *,
    manifest: dict,
    page_ids: set,
    region_by_id: dict,
    object_ids: set,
) -> None:
    if (
        source.get("entity_id") != manifest["material_id"]
        or source.get("revision_id") != manifest["source_version_id"]
        or source.get("content_hash") != manifest["source_hash"]
    ):
        _invalid_result("material source reference escaped its original source", scope="asset")
    selector = source.get("selector")
    if not isinstance(selector, dict):
        _invalid_result("material source reference has no selector", scope="asset")
    _validate_material_selector(
        selector,
        manifest=manifest,
        page_ids=page_ids,
        region_by_id=region_by_id,
        object_ids=object_ids,
    )


def _iter_source_refs(value):
    if isinstance(value, dict):
        if {"entity_id", "revision_id", "content_hash", "selector"} <= set(value):
            yield value
        for nested in value.values():
            yield from _iter_source_refs(nested)
    elif isinstance(value, list):
        for nested in value:
            yield from _iter_source_refs(nested)


def validate_manifest(manifest: dict, staging_root: Path, *, check_resources: bool = True) -> None:
    required = {
        "format", "format_version", "material_id", "source_version_id", "parse_version_id",
        "parent_parse_version_id", "source_hash", "engine_build", "parse_profile", "requested_pages",
        "pages", "regions", "objects", "resources", "corrections_applied", "unresolved_conflicts",
        "provenance", "completion", "search_units", "search_coverage",
    }
    missing = sorted(required - set(manifest))
    if missing:
        raise EngineError("INVALID_RESULT", "manifest is missing required fields", scope="asset", details={"fields": ",".join(missing)})
    if manifest["format"] != MATERIAL_FORMAT or manifest["format_version"] != MATERIAL_FORMAT_VERSION:
        raise EngineError("UNSUPPORTED_VERSION", "manifest format version is not focaus.material/2.0.0", scope="asset")
    source_page_indices = [page.get("source_page_index") for page in manifest["pages"]]
    if len(source_page_indices) != len(set(source_page_indices)):
        _invalid_result("manifest contains duplicate source page indices", scope="asset")
    ids = _all_ids(manifest)
    if len(ids) != len(set(ids)):
        raise EngineError("INVALID_RESULT", "manifest contains duplicate page/region/object/resource IDs", scope="asset")
    page_ids = {page["page_id"] for page in manifest["pages"]}
    region_ids = {region["region_id"] for region in manifest["regions"]}
    region_by_id = {region["region_id"]: region for region in manifest["regions"]}
    object_ids = {obj["object_id"] for obj in manifest["objects"]}
    resource_ids = {resource["resource_id"] for resource in manifest["resources"]}
    allowed_resource_owners = page_ids | region_ids | object_ids | {manifest["material_id"]}
    relative_paths = set()
    if check_resources:
        for resource in manifest["resources"]:
            relative = safe_relative_path(resource["relative_path"])
            if relative in relative_paths:
                _invalid_result("manifest resources reuse a relative path", scope="asset", details={"relative_path": relative})
            relative_paths.add(relative)
            path = ensure_within(staging_root, staging_root / relative, field_name="resource")
            if not path.is_file():
                raise EngineError("INVALID_RESULT", "manifest resource is missing", scope="asset", details={"resource_id": resource["resource_id"]})
            data = path.read_bytes()
            if len(data) != resource["byte_length"] or sha256_bytes(data) != resource["sha256"]:
                raise EngineError("INVALID_RESULT", "manifest resource hash or length mismatch", scope="asset", details={"resource_id": resource["resource_id"]})
    for resource in manifest["resources"]:
        if not set(resource.get("owner_refs", [])) <= allowed_resource_owners:
            _invalid_result("resource owner is outside the package", scope="asset", details={"resource_id": resource["resource_id"]})
    for page in manifest["pages"]:
        if page.get("preview_resource_id") is not None and page["preview_resource_id"] not in resource_ids:
            raise EngineError("INVALID_RESULT", "page preview resource is outside package", scope="page")
        if not set(page.get("region_ids", [])) <= region_ids or not set(page.get("reading_order", [])) <= object_ids | region_ids:
            raise EngineError("INVALID_RESULT", "page closure is incomplete", scope="page", details={"page_id": page["page_id"]})
    for region in manifest["regions"]:
        if region["page_id"] not in page_ids or not set(region.get("object_ids", [])) <= object_ids:
            raise EngineError("INVALID_RESULT", "region closure is incomplete", scope="page", details={"region_id": region["region_id"]})
        if region.get("parent_region_id") is not None and region["parent_region_id"] not in region_ids:
            raise EngineError("INVALID_RESULT", "region parent is missing", scope="page", details={"region_id": region["region_id"]})
    for obj in manifest["objects"]:
        if not set(obj.get("region_ids", [])) <= region_ids or not set(obj.get("children", [])) <= object_ids:
            raise EngineError("INVALID_RESULT", "object closure is incomplete", scope="page", details={"object_id": obj["object_id"]})
        if obj.get("type") == "image" and obj.get("content", {}).get("resource_id") not in resource_ids:
            raise EngineError("INVALID_RESULT", "image resource is missing", scope="page", details={"object_id": obj["object_id"]})
        for source in _iter_source_refs(obj):
            _validate_source_ref(
                source,
                manifest=manifest,
                page_ids=page_ids,
                region_by_id=region_by_id,
                object_ids=object_ids,
            )
    for unit in manifest["search_units"]:
        if unit.get("owner_entity_id") not in object_ids:
            raise EngineError("INVALID_RESULT", "search unit owner is missing", scope="page")
        display_text = unit.get("display_text")
        if not isinstance(display_text, str):
            _invalid_result("search unit display text is not a string")
        if unit.get("content_hash") != text_hash(display_text):
            _invalid_result("search unit text hash does not match its display text")
        alternate_anchors = unit.get("alternate_anchors", [])
        if not isinstance(alternate_anchors, list):
            _invalid_result("search unit alternate anchors are not a list")
        anchors = [unit.get("anchor")] + alternate_anchors
        for anchor in anchors:
            if not isinstance(anchor, dict):
                _invalid_result("search unit contains an invalid anchor")
            if (
                anchor.get("owner_entity_id") != unit.get("owner_entity_id")
                or anchor.get("revision_id") != unit.get("revision_id")
            ):
                _invalid_result("search anchor owner or revision does not match unit")
            scalar_range = anchor.get("scalar_range", {})
            start = scalar_range.get("start")
            end = scalar_range.get("end")
            if (
                not isinstance(start, int)
                or not isinstance(end, int)
                or start < 0
                or end < start
                or end > len(display_text)
            ):
                _invalid_result("search anchor scalar range is outside its display text")
            selector = anchor.get("selector")
            if not isinstance(selector, dict):
                _invalid_result("search anchor contains no selector")
            _validate_material_selector(
                selector,
                manifest=manifest,
                page_ids=page_ids,
                region_by_id=region_by_id,
                object_ids=object_ids,
                owner_entity_id=unit.get("owner_entity_id"),
            )
    for source in _iter_source_refs(manifest.get("provenance", {})):
        _validate_source_ref(
            source,
            manifest=manifest,
            page_ids=page_ids,
            region_by_id=region_by_id,
            object_ids=object_ids,
        )
    coverage = manifest["search_coverage"]
    if not set(coverage.get("requested_pages", [])) <= page_ids or not set(coverage.get("processed_pages", [])) <= page_ids:
        raise EngineError("INVALID_RESULT", "search coverage page closure is incomplete", scope="page")
    if manifest["completion"] == "complete" and coverage.get("state") != "complete":
        raise EngineError("INVALID_RESULT", "complete manifest has incomplete search coverage", scope="asset")
    if manifest["parse_profile"] == "deep" and manifest["completion"] == "complete" and any(obj.get("status") != "complete" for obj in manifest["objects"]):
        raise EngineError("INVALID_RESULT", "deep complete manifest contains partial objects", scope="asset")


def write_manifest(staging_root: Path, manifest: dict) -> Path:
    """Write the replaceable recovery/head copy.

    Published snapshots use :func:`write_snapshot`; this head file may move
    forward as a task checkpoints pages and is never used as the immutable
    identity of a parse version.
    """
    path = ensure_within(staging_root, staging_root / "manifest.json", field_name="manifest")
    _write_json(path, manifest)
    return path


def snapshot_path(staging_root: Path, parse_version_id: str) -> Path:
    if not is_uuid(parse_version_id):
        raise EngineError("INVALID_ARGUMENT", "snapshot parse_version_id must be a UUID", scope="asset")
    return ensure_within(
        staging_root,
        staging_root / "snapshots" / (str(parse_version_id) + ".json"),
        field_name="snapshot",
    )


def read_snapshot(staging_root: Path, parse_version_id: str) -> Optional[dict]:
    path = snapshot_path(staging_root, parse_version_id)
    if not path.is_file():
        return None
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, ValueError) as exc:
        raise EngineError(
            "CORRUPT_ASSET",
            "parse snapshot is not valid JSON",
            scope="asset",
            details={"parse_version_id": str(parse_version_id), "exception": type(exc).__name__},
        )
    if not isinstance(value, dict):
        raise EngineError("CORRUPT_ASSET", "parse snapshot is not an object", scope="asset")
    return value


def validate_snapshot_header(
    manifest: dict,
    *,
    parse_version_id: str,
    material_id: str,
    source_version_id: Optional[str] = None,
    source_hash: Optional[str] = None,
) -> None:
    if not isinstance(manifest, dict):
        raise EngineError("CORRUPT_ASSET", "parent snapshot is not an object", scope="asset")
    if manifest.get("format") != MATERIAL_FORMAT or manifest.get("format_version") != MATERIAL_FORMAT_VERSION:
        raise EngineError("UNSUPPORTED_VERSION", "parent snapshot has an unsupported material format", scope="asset")
    if manifest.get("parse_version_id") != parse_version_id:
        raise EngineError("IDEMPOTENCY_MISMATCH", "parent snapshot ID does not match the requested parent", scope="task")
    if manifest.get("material_id") != material_id:
        raise EngineError("CONFLICT", "parent snapshot belongs to another material", scope="asset")
    if source_version_id is not None and manifest.get("source_version_id") != source_version_id:
        raise EngineError("STALE_INPUT", "parent snapshot belongs to another source version", scope="asset")
    if source_hash is not None and manifest.get("source_hash") != source_hash:
        raise EngineError("STALE_INPUT", "parent snapshot was created from another source", scope="asset")


def write_snapshot(
    staging_root: Path,
    manifest: dict,
    *,
    externally_verified_parent_ids: Optional[Sequence[str]] = None,
) -> Path:
    """Publish an immutable parse snapshot without replacing an old file."""
    parse_version_id = manifest.get("parse_version_id")
    path = snapshot_path(staging_root, parse_version_id)
    parent_id = manifest.get("parent_parse_version_id")
    external_ids = set(externally_verified_parent_ids or [])
    if parent_id is not None:
        parent_path = snapshot_path(staging_root, parent_id)
        if not parent_path.is_file() and parent_id not in external_ids:
            raise EngineError(
                "SOURCE_UNAVAILABLE",
                "parse snapshot parent has not been published",
                retryable=True,
                scope="asset",
                details={"parent_parse_version_id": str(parent_id)},
            )
    validate_manifest(manifest, staging_root)
    if path.is_file():
        existing = read_snapshot(staging_root, parse_version_id)
        if existing == manifest:
            return path
        raise EngineError(
            "IDEMPOTENCY_MISMATCH",
            "parse_version_id already identifies a different snapshot",
            scope="task",
            details={"parse_version_id": str(parse_version_id)},
        )
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_name(path.name + ".tmp")
    temporary.write_text(json.dumps(manifest, ensure_ascii=False, sort_keys=True, indent=2) + "\n", encoding="utf-8")
    try:
        os.link(str(temporary), str(path))
    except FileExistsError:
        existing = read_snapshot(staging_root, parse_version_id)
        if existing != manifest:
            raise EngineError("IDEMPOTENCY_MISMATCH", "snapshot publication raced with a different payload", scope="task")
    finally:
        try:
            temporary.unlink()
        except FileNotFoundError:
            pass
    return path


def write_checkpoint(staging_root: Path, checkpoint: dict) -> Path:
    path = ensure_within(staging_root, staging_root / "checkpoint.json", field_name="checkpoint")
    _write_json(path, checkpoint)
    return path


def read_checkpoint(staging_root: Path) -> Optional[dict]:
    path = staging_root / "checkpoint.json"
    if not path.is_file():
        return None
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, ValueError):
        return None
    return value if isinstance(value, dict) else None
