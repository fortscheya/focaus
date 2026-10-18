"""Shared manifest builders and deterministic, host-confirmable identities."""

from __future__ import annotations

import hashlib
import html
import json
import mimetypes
import os
import re
import uuid
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Dict, Iterable, List, Mapping, MutableMapping, Optional, Sequence, Tuple

from .errors import EngineError


CONTRACT_VERSION = "2.0.0"
MATERIAL_FORMAT = "focaus.material"
MATERIAL_FORMAT_VERSION = "2.0.0"
SEARCH_PROFILE = "nfc_casefold_ws_v1"
EXTRACTION_VERSION = "1.0.0"
ID_NAMESPACE = uuid.UUID("8ed0c52c-bd80-4e75-8a76-3fd2e643b9f9")


def sha256_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def sha256_path(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def canonical_json_bytes(value: Any) -> bytes:
    """Stable JSON for local correction guards.

    The public contract uses RFC 8785.  The engine has no dependency on the
    contract validator at runtime, so this is the same useful subset for
    correction values (strings, booleans, numbers, arrays and objects).  The
    host still validates the final package with the generated contract.
    """

    return json.dumps(
        value,
        ensure_ascii=False,
        allow_nan=False,
        sort_keys=True,
        separators=(",", ":"),
    ).encode("utf-8")


def content_hash(value: Any) -> str:
    return sha256_bytes(canonical_json_bytes(value))


def text_hash(text: str) -> str:
    return sha256_bytes(text.encode("utf-8"))


def is_uuid(value: Any) -> bool:
    try:
        uuid.UUID(str(value))
    except (ValueError, TypeError, AttributeError):
        return False
    return True


def require_uuid(value: Any, field_name: str) -> str:
    if not is_uuid(value):
        raise EngineError(
            "INVALID_ARGUMENT",
            "{} must be a UUID".format(field_name),
            scope="command",
            details={"field": field_name},
        )
    return str(value)


def clamp_bbox(values: Sequence[float]) -> List[float]:
    if len(values) != 4:
        raise EngineError("INVALID_ARGUMENT", "bbox must have four values", scope="page")
    x, y, width, height = (float(v) for v in values)
    x = max(0.0, min(1.0, x))
    y = max(0.0, min(1.0, y))
    width = max(0.0, min(1.0 - x, width))
    height = max(0.0, min(1.0 - y, height))
    return [x, y, width, height]


class IdentityAllocator:
    """Resolve host-provided IDs and generate clearly provisional candidates."""

    def __init__(
        self,
        material_id: str,
        source_version_id: str,
        identity_pool: Optional[Mapping[str, str]] = None,
    ) -> None:
        self.material_id = material_id
        self.source_version_id = source_version_id
        self.identity_pool = dict(identity_pool or {})
        self.namespace = uuid.uuid5(
            ID_NAMESPACE,
            "{}:{}".format(material_id, source_version_id),
        )
        self._resolved: Dict[Tuple[str, str], str] = {}
        self._candidate_keys: List[str] = []
        self._host_keys: List[str] = []

    def _pool_value(self, kind: str, key: str) -> Optional[str]:
        candidates = ("{}:{}".format(kind, key), "{}/{}".format(kind, key), key)
        for candidate in candidates:
            if candidate in self.identity_pool:
                value = self.identity_pool[candidate]
                if not is_uuid(value):
                    raise EngineError(
                        "INVALID_ARGUMENT",
                        "identity_pool value is not a UUID",
                        scope="command",
                        details={"identity_key": candidate},
                    )
                self._host_keys.append(candidate)
                return str(value)
        return None

    def id(self, kind: str, key: str) -> str:
        cache_key = (str(kind), str(key))
        if cache_key in self._resolved:
            return self._resolved[cache_key]
        value = self._pool_value(str(kind), str(key))
        if value is None:
            value = str(uuid.uuid5(self.namespace, "{}:{}".format(kind, key)))
            self._candidate_keys.append("{}:{}".format(kind, key))
        self._resolved[cache_key] = value
        return value

    def unique_id(self, kind: str, key: str, existing: Iterable[str]) -> str:
        """Allocate a publication identity, preferring a host pool entry.

        Parse snapshots are publication events, so an engine fallback derives a
        request-scoped UUID from the task and sequence key and records it as a
        candidate.  This keeps recovery idempotent when a process stops after
        publishing a snapshot but before flushing its checkpoint.  A host
        supplied value remains authoritative.
        """
        existing_set = {str(value) for value in existing}
        pooled = self._pool_value(kind, key)
        if pooled is not None:
            if pooled in existing_set:
                raise EngineError(
                    "IDEMPOTENCY_MISMATCH",
                    "identity_pool publication ID is already in use",
                    scope="task",
                    details={"identity_key": "{}:{}".format(kind, key)},
                )
            self._resolved[(str(kind), str(key))] = pooled
            return pooled
        for attempt in range(16):
            suffix = "{}:{}".format(kind, key) if attempt == 0 else "{}:{}:{}".format(kind, key, attempt)
            candidate = str(uuid.uuid5(self.namespace, suffix))
            if candidate not in existing_set:
                self._resolved[(str(kind), str(key))] = candidate
                self._candidate_keys.append("{}:{}".format(kind, key))
                return candidate
        raise EngineError("IDEMPOTENCY_MISMATCH", "could not allocate a unique publication ID", scope="task")

    @property
    def used_candidates(self) -> List[str]:
        return list(dict.fromkeys(self._candidate_keys))

    @property
    def used_host_keys(self) -> List[str]:
        return list(dict.fromkeys(self._host_keys))

    def provenance(self) -> dict:
        if self.used_candidates:
            return {
                "mode": "engine_candidate_requires_host_confirmation",
                "host_confirmed_keys": self.used_host_keys,
                "candidate_keys": self.used_candidates,
            }
        return {"mode": "host_preallocated_or_confirmed", "host_confirmed_keys": self.used_host_keys}


def ensure_within(root: Path, candidate: Path, *, field_name: str) -> Path:
    root = root.resolve()
    candidate = candidate.expanduser().resolve()
    try:
        candidate.relative_to(root)
    except ValueError:
        raise EngineError(
            "PERMISSION_DENIED",
            "{} must remain inside the staging directory".format(field_name),
            scope="asset",
            details={"field": field_name},
        )
    return candidate


def safe_relative_path(path: str) -> str:
    path = str(path)
    if "\\" in path:
        raise EngineError("INVALID_ARGUMENT", "resource path contains a backslash", scope="asset")
    if path.startswith("/"):
        raise EngineError("INVALID_ARGUMENT", "resource path escapes package root", scope="asset")
    parts = path.split("/")
    if ".." in parts:
        raise EngineError("INVALID_ARGUMENT", "resource path escapes package root", scope="asset")
    if any(part in ("", ".") for part in parts):
        raise EngineError("INVALID_ARGUMENT", "resource path contains an empty component", scope="asset")
    return path


_MEDIA_EXTENSIONS = {
    "image/png": ".png",
    "image/jpeg": ".jpg",
    "image/gif": ".gif",
    "image/webp": ".webp",
    "image/svg+xml": ".svg",
    "application/pdf": ".pdf",
    "application/octet-stream": ".bin",
}


class ResourceStore:
    """Own package-local resource bytes and their contract records."""

    def __init__(self, staging_root: Path, ids: IdentityAllocator) -> None:
        self.staging_root = staging_root
        self.resources_root = staging_root / "resources"
        self.resources_root.mkdir(parents=True, exist_ok=True)
        self.ids = ids
        self._records: Dict[str, dict] = {}
        self._key_to_id: Dict[str, str] = {}
        self._bytes_by_key: Dict[str, bytes] = {}

    def add_bytes(
        self,
        key: str,
        data: bytes,
        media_type: str,
        role: str,
        owner_refs: Iterable[str],
    ) -> str:
        key = str(key)
        owner_refs = list(dict.fromkeys(str(v) for v in owner_refs))
        if key in self._key_to_id:
            resource_id = self._key_to_id[key]
            if self._bytes_by_key[key] != data:
                raise EngineError(
                    "CORRUPT_ASSET",
                    "resource key was reused with different bytes",
                    scope="asset",
                    details={"resource_key": key},
                )
            record = self._records[resource_id]
            record["owner_refs"] = list(dict.fromkeys(record["owner_refs"] + owner_refs))
            return resource_id

        resource_id = self.ids.id("resource", key)
        extension = _MEDIA_EXTENSIONS.get(media_type) or mimetypes.guess_extension(media_type) or ".bin"
        relative_path = safe_relative_path("resources/{}{}".format(resource_id, extension))
        destination = ensure_within(
            self.staging_root,
            self.staging_root / relative_path,
            field_name="resource",
        )
        destination.parent.mkdir(parents=True, exist_ok=True)
        existing_record = self._records.get(resource_id)
        if destination.exists():
            if not destination.is_file():
                raise EngineError(
                    "CORRUPT_ASSET",
                    "resource destination is not a regular file",
                    scope="asset",
                    details={"resource_id": resource_id},
                )
            existing_data = destination.read_bytes()
            if existing_data != data:
                raise EngineError(
                    "CORRUPT_ASSET",
                    "immutable resource ID was reused with different bytes",
                    scope="asset",
                    details={"resource_id": resource_id},
                )
            if existing_record is None:
                existing_record = {
                    "resource_id": resource_id,
                    "media_type": media_type,
                    "byte_length": len(data),
                    "sha256": sha256_bytes(data),
                    "relative_path": relative_path,
                    "role": role,
                    "owner_refs": owner_refs,
                }
                self._records[resource_id] = existing_record
            else:
                existing_record["owner_refs"] = list(dict.fromkeys(existing_record.get("owner_refs", []) + owner_refs))
            self._key_to_id[key] = resource_id
            self._bytes_by_key[key] = data
            return resource_id
        temporary = destination.with_name(destination.name + ".tmp")
        temporary.write_bytes(data)
        os.replace(str(temporary), str(destination))
        record = {
            "resource_id": resource_id,
            "media_type": media_type,
            "byte_length": len(data),
            "sha256": sha256_bytes(data),
            "relative_path": relative_path,
            "role": role,
            "owner_refs": owner_refs,
        }
        self._key_to_id[key] = resource_id
        self._bytes_by_key[key] = data
        self._records[resource_id] = record
        return resource_id

    def add_file(
        self,
        key: str,
        path: Path,
        media_type: str,
        role: str,
        owner_refs: Iterable[str],
    ) -> str:
        return self.add_bytes(key, path.read_bytes(), media_type, role, owner_refs)

    def load_existing(self, records: Iterable[dict]) -> None:
        """Load resources from a previous checkpoint after verifying their bytes."""
        for record in records:
            relative_path = safe_relative_path(record["relative_path"])
            path = ensure_within(self.staging_root, self.staging_root / relative_path, field_name="resource")
            if not path.is_file():
                raise EngineError("SOURCE_UNAVAILABLE", "checkpoint resource is missing", retryable=True, scope="asset", details={"resource_id": str(record.get("resource_id"))})
            data = path.read_bytes()
            if len(data) != int(record.get("byte_length", -1)) or sha256_bytes(data) != record.get("sha256"):
                raise EngineError("CORRUPT_ASSET", "checkpoint resource failed hash verification", scope="asset", details={"resource_id": str(record.get("resource_id"))})
            resource_id = str(record["resource_id"])
            self._records[resource_id] = dict(record)
            self._key_to_id[relative_path] = resource_id
            self._bytes_by_key[relative_path] = data

    def records(self) -> List[dict]:
        return [self._records[key] for key in sorted(self._records)]


@dataclass
class ExtractionContext:
    source_path: Path
    source_hash: str
    material_id: str
    source_version_id: str
    parse_version_id: str
    parent_parse_version_id: Optional[str]
    profile: str
    staging_root: Path
    ids: IdentityAllocator
    resources: ResourceStore
    warnings: List[str] = field(default_factory=list)
    formula_evidence: List[dict] = field(default_factory=list)
    rendering: Dict[str, Any] = field(default_factory=dict)

    def add_warning(self, message: str) -> None:
        if message not in self.warnings:
            self.warnings.append(message)


def page_selector(
    ctx: ExtractionContext,
    page_id: str,
    region_id: str,
    bbox: Sequence[float],
) -> dict:
    return {
        "kind": "material_region",
        "material_id": ctx.material_id,
        "source_version_id": ctx.source_version_id,
        "parse_version_id": ctx.parse_version_id,
        "page_id": page_id,
        "region_id": region_id,
        "bbox": clamp_bbox(bbox),
        "coordinate_space": "rotated_page_normalized",
    }


def object_selector(object_id: str, element_ids: Iterable[str]) -> dict:
    return {"kind": "object", "object_id": object_id, "element_ids": list(element_ids)}


def source_ref(ctx: ExtractionContext, page_id: str, region_id: str, bbox: Sequence[float]) -> dict:
    return {
        "entity_id": ctx.material_id,
        "revision_id": ctx.source_version_id,
        "content_hash": ctx.source_hash,
        "selector": page_selector(ctx, page_id, region_id, bbox),
    }


def region_record(
    ctx: ExtractionContext,
    page_index: int,
    page_id: str,
    key: str,
    bbox: Sequence[float],
    object_ids: Iterable[str],
    semantic_role: str,
    parent_region_id: Optional[str] = None,
) -> dict:
    object_id_list = list(object_ids)
    return {
        "region_id": ctx.ids.id("region", "{}:{}:{}".format(page_index, key, len(object_id_list))),
        "page_id": page_id,
        "bbox": clamp_bbox(bbox),
        "object_ids": object_id_list,
        "semantic_role": semantic_role,
        "parent_region_id": parent_region_id,
    }


def layout(bounds: Sequence[float], z_order: int = 0, group_id: Optional[str] = None) -> dict:
    return {
        "bounds": clamp_bbox(bounds),
        "z_order": int(z_order),
        "group_id": group_id,
        "constraints": [],
    }


def base_object(
    ctx: ExtractionContext,
    page_index: int,
    key: str,
    object_type: str,
    region_id: str,
    bounds: Sequence[float],
    source: dict,
    *,
    extraction: str,
    editability: str,
    status: str,
    content: dict,
    z_order: int = 0,
    children: Optional[Iterable[str]] = None,
) -> dict:
    object_id = ctx.ids.id("object", "{}:{}".format(page_index, key))
    return {
        "object_id": object_id,
        "revision_id": ctx.ids.id("object_revision", "{}:{}".format(page_index, key)),
        "source_refs": [source],
        "region_ids": [region_id],
        "layout": layout(bounds, z_order),
        "extraction": extraction,
        "editability": editability,
        "status": status,
        "children": list(children or []),
        "type": object_type,
        "content": content,
    }


def search_anchor(
    owner_entity_id: str,
    revision_id: str,
    selector: dict,
    field_path: str,
    text: str,
    *,
    audio_spans: Optional[Iterable[str]] = None,
    fallback: Optional[str] = None,
) -> dict:
    return {
        "owner_entity_id": owner_entity_id,
        "revision_id": revision_id,
        "selector": selector,
        "field_path": field_path,
        "scalar_range": {"start": 0, "end": len(text)},
        "audio_spans": list(audio_spans or []),
        "fallback": fallback,
    }


def add_search_unit(
    ctx: ExtractionContext,
    units: List[dict],
    *,
    page_index: int,
    object_value: dict,
    text: str,
    field_path: str,
    element_ids: Iterable[str],
    page_id: str,
    region_id: str,
    bbox: Sequence[float],
    equivalence_group_id: Optional[str] = None,
    alternate: bool = True,
) -> None:
    object_id = object_value["object_id"]
    revision_id = object_value["revision_id"]
    selector = object_selector(object_id, element_ids)
    fallback = "page:{};region:{};object:{}".format(page_id, region_id, object_id)
    anchor = search_anchor(object_id, revision_id, selector, field_path, text, fallback=fallback)
    alternate_anchors = []
    if alternate:
        alternate_anchors.append(
            search_anchor(
                object_id,
                revision_id,
                page_selector(ctx, page_id, region_id, bbox),
                field_path,
                text,
                fallback=fallback,
            )
        )
    units.append(
        {
            "unit_key": "page:{}:object:{}:{}".format(page_index, object_id, field_path),
            "owner_entity_id": object_id,
            "revision_id": revision_id,
            "content_hash": text_hash(text),
            "display_text": text,
            "field_path": field_path,
            "anchor": anchor,
            "equivalence_group_id": equivalence_group_id,
            "alternate_anchors": alternate_anchors,
            "source_role": "material_supplement",
            "extraction_version": EXTRACTION_VERSION,
        }
    )


def make_text_runs(ctx: ExtractionContext, page_index: int, key: str, text: str) -> Tuple[str, List[dict]]:
    block_id = ctx.ids.id("block", "{}:{}".format(page_index, key))
    runs = [
        {
            "element_id": ctx.ids.id("element", "{}:{}:0".format(page_index, key)),
            "text": text,
        }
    ]
    return block_id, runs


_FORMULA_HINT = re.compile(r"(?:[=∑∫√∞≤≥→←↔∂Δ]|\\(?:frac|sum|int|sqrt|begin|end)|\b(?:arg\s*max|sin|cos|tan|log|exp)\b)")
_VARIABLE = re.compile(r"(?<![A-Za-z])([A-Za-z][A-Za-z0-9_]{0,31})(?![A-Za-z0-9_])")


def looks_like_formula(text: str) -> bool:
    return bool(_FORMULA_HINT.search(text))


def formula_variables(text: str) -> List[str]:
    excluded = {"the", "and", "for", "with", "where", "when", "from", "into", "this", "that"}
    return sorted({match.group(1) for match in _VARIABLE.finditer(text) if match.group(1).lower() not in excluded})


def record_formula_evidence(
    ctx: ExtractionContext,
    *,
    page_id: str,
    region_id: str,
    object_value: dict,
    latex: str,
) -> None:
    variables = formula_variables(latex)
    ctx.formula_evidence.append(
        {
            "object_id": object_value["object_id"],
            "page_id": page_id,
            "region_id": region_id,
            "field_path": "content.latex",
            "latex": latex,
            "variables": [
                {
                    "symbol": symbol,
                    "scope_id": page_id,
                    "meaning": None,
                    "expression": latex,
                    "constraints": [],
                    "value": None,
                    "status": "semantic_meaning_requires_host_or_AI_evidence",
                }
                for symbol in variables
            ],
            "source_ref": object_value["source_refs"][0],
            "semantic_status": "candidate_only",
        }
    )


def svg_placeholder(width: float, height: float, text: str, *, title: str = "Focaus page preview") -> bytes:
    safe_text = html.escape(text[:2000])
    safe_title = html.escape(title)
    svg = (
        '<?xml version="1.0" encoding="UTF-8"?>'
        '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {w} {h}">'
        '<title>{title}</title><rect width="100%" height="100%" fill="white"/>'
        '<text x="24" y="42" font-family="sans-serif" font-size="18" fill="#222">{text}</text>'
        "</svg>"
    ).format(w=max(1, int(width)), h=max(1, int(height)), title=safe_title, text=safe_text.replace("\n", " &#10;"))
    return svg.encode("utf-8")


def media_type_for_path(path: Path) -> str:
    guessed, _ = mimetypes.guess_type(str(path))
    return guessed or "application/octet-stream"
