"""Material manifest assembly, closure checks and staging writes."""

from __future__ import annotations

import json
import os
from pathlib import Path
from typing import Iterable, List, Optional, Sequence

from .common import (
    CONTRACT_VERSION,
    EXTRACTION_VERSION,
    MATERIAL_FORMAT,
    MATERIAL_FORMAT_VERSION,
    SEARCH_PROFILE,
    ExtractionContext,
    ensure_within,
    safe_relative_path,
    sha256_bytes,
)
from .errors import EngineError
from .models import PageDraft


def _write_json(path: Path, value: dict) -> None:
    temporary = path.with_name(path.name + ".tmp")
    temporary.write_text(json.dumps(value, ensure_ascii=False, sort_keys=True, indent=2) + "\n", encoding="utf-8")
    os.replace(str(temporary), str(path))


def _coverage(ctx: ExtractionContext, drafts: Sequence[PageDraft]) -> dict:
    requested_pages = [draft.page_id for draft in drafts]
    processed_pages = [draft.page_id for draft in drafts if draft.processed_for_search]
    unprocessed = [area for draft in drafts for area in draft.coverage_unprocessed]
    failed = [area for draft in drafts for area in draft.coverage_failed]
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
) -> dict:
    draft_list = sorted(list(drafts), key=lambda draft: draft.index)
    coverage = _coverage(ctx, draft_list)
    pages = [draft.page for draft in draft_list]
    regions = [region for draft in draft_list for region in draft.regions]
    objects = [obj for draft in draft_list for obj in draft.objects]
    search_units = [unit for draft in draft_list for unit in draft.search_units]
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
    manifest = {
        "format": MATERIAL_FORMAT,
        "format_version": MATERIAL_FORMAT_VERSION,
        "material_id": ctx.material_id,
        "source_version_id": ctx.source_version_id,
        "parse_version_id": ctx.parse_version_id,
        "parent_parse_version_id": ctx.parent_parse_version_id,
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


def validate_manifest(manifest: dict, staging_root: Path) -> None:
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
    ids = _all_ids(manifest)
    if len(ids) != len(set(ids)):
        raise EngineError("INVALID_RESULT", "manifest contains duplicate page/region/object/resource IDs", scope="asset")
    page_ids = {page["page_id"] for page in manifest["pages"]}
    region_ids = {region["region_id"] for region in manifest["regions"]}
    object_ids = {obj["object_id"] for obj in manifest["objects"]}
    resource_ids = {resource["resource_id"] for resource in manifest["resources"]}
    for resource in manifest["resources"]:
        relative = safe_relative_path(resource["relative_path"])
        path = ensure_within(staging_root, staging_root / relative, field_name="resource")
        if not path.is_file():
            raise EngineError("INVALID_RESULT", "manifest resource is missing", scope="asset", details={"resource_id": resource["resource_id"]})
        data = path.read_bytes()
        if len(data) != resource["byte_length"] or sha256_bytes(data) != resource["sha256"]:
            raise EngineError("INVALID_RESULT", "manifest resource hash or length mismatch", scope="asset", details={"resource_id": resource["resource_id"]})
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
    for unit in manifest["search_units"]:
        if unit.get("owner_entity_id") not in object_ids:
            raise EngineError("INVALID_RESULT", "search unit owner is missing", scope="page")
        anchor = unit.get("anchor", {})
        if anchor.get("owner_entity_id") != unit.get("owner_entity_id"):
            raise EngineError("INVALID_RESULT", "search anchor owner does not match unit", scope="page")
    coverage = manifest["search_coverage"]
    if not set(coverage.get("requested_pages", [])) <= page_ids or not set(coverage.get("processed_pages", [])) <= page_ids:
        raise EngineError("INVALID_RESULT", "search coverage page closure is incomplete", scope="page")
    if manifest["completion"] == "complete" and coverage.get("state") != "complete":
        raise EngineError("INVALID_RESULT", "complete manifest has incomplete search coverage", scope="asset")
    if manifest["parse_profile"] == "deep" and manifest["completion"] == "complete" and any(obj.get("status") != "complete" for obj in manifest["objects"]):
        raise EngineError("INVALID_RESULT", "deep complete manifest contains partial objects", scope="asset")


def write_manifest(staging_root: Path, manifest: dict) -> Path:
    path = ensure_within(staging_root, staging_root / "manifest.json", field_name="manifest")
    _write_json(path, manifest)
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
