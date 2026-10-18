"""Replay user material corrections on a fresh candidate parse."""

from __future__ import annotations

from copy import deepcopy
from typing import Any, Dict, Iterable, List, Optional, Tuple

from .common import ExtractionContext, content_hash, text_hash


def _selector_object_ids(selector: dict, objects: Iterable[dict]) -> List[str]:
    object_list = list(objects)
    kind = selector.get("kind")
    if kind == "object":
        object_id = selector.get("object_id")
        return [object_id] if object_id and any(o.get("object_id") == object_id for o in object_list) else []
    if kind == "text":
        block_id = selector.get("block_id")
        if not block_id:
            return []
        return [o["object_id"] for o in object_list if o.get("type") == "text" and o.get("content", {}).get("block_id") == block_id]
    if kind == "material_region":
        region_id = selector.get("region_id")
        if not region_id:
            return []
        return [o["object_id"] for o in object_list if region_id in o.get("region_ids", [])]
    return []


def _object_text(obj: dict) -> Optional[str]:
    if obj.get("type") == "text":
        return "".join(run.get("text", "") for run in obj.get("content", {}).get("runs", []))
    if obj.get("type") == "formula":
        return obj.get("content", {}).get("latex", "")
    return None


def _hash_matches(current: Any, expected: str) -> bool:
    if expected in (content_hash(current), text_hash(current) if isinstance(current, str) else ""):
        return True
    # A host may have used the hash of the complete JSON field rather than the
    # scalar text.  Accepting this equivalent guard keeps correction replay
    # compatible while still rejecting a changed base.
    return False


def _replacement_value(replacement: Any, key: str = "value") -> Any:
    if isinstance(replacement, dict) and key in replacement:
        return replacement[key]
    return replacement


def _apply_to_text(obj: dict, replacement: Any) -> Tuple[Any, bool]:
    current = _object_text(obj)
    if current is None:
        return None, False
    value = _replacement_value(replacement, "text")
    if not isinstance(value, str):
        return current, False
    if obj.get("type") == "formula":
        obj["content"]["latex"] = value
    else:
        runs = obj["content"].get("runs", [])
        if runs:
            runs[0]["text"] = value
            for run in runs[1:]:
                run["text"] = ""
        else:
            obj["content"]["runs"] = [{"element_id": obj["object_id"], "text": value}]
    return value, True


def _find_cell(obj: dict, cell_id: Optional[str]) -> Optional[dict]:
    if obj.get("type") != "table":
        return None
    for cell in obj.get("content", {}).get("cells", []):
        if cell_id is None or cell.get("cell_id") == cell_id:
            return cell
    return None


def _find_point(obj: dict, point_id: Optional[str]) -> Optional[dict]:
    if obj.get("type") != "data_chart":
        return None
    for series in obj.get("content", {}).get("series", []):
        for point in series.get("points", []):
            if point_id is None or point.get("point_id") == point_id:
                return point
    return None


def _find_edge(obj: dict, edge_id: Optional[str]) -> Optional[dict]:
    if obj.get("type") != "structure_diagram":
        return None
    for edge in obj.get("content", {}).get("edges", []):
        if edge_id is None or edge.get("edge_id") == edge_id:
            return edge
    return None


def _refresh_unit(unit: dict, value: str) -> None:
    unit["display_text"] = value
    unit["content_hash"] = text_hash(value)
    for anchor_key in ("anchor",):
        anchor = unit.get(anchor_key)
        if anchor:
            anchor["scalar_range"] = {"start": 0, "end": len(value)}
    for anchor in unit.get("alternate_anchors", []):
        anchor["scalar_range"] = {"start": 0, "end": len(value)}


def _refresh_object_units(units: List[dict], obj: dict, old_text: Optional[str] = None) -> None:
    object_id = obj.get("object_id")
    for unit in units:
        if unit.get("owner_entity_id") != object_id:
            continue
        unit["revision_id"] = obj.get("revision_id", unit.get("revision_id"))
        if unit.get("anchor"):
            unit["anchor"]["revision_id"] = obj.get("revision_id", unit["anchor"].get("revision_id"))
        for alternate in unit.get("alternate_anchors", []):
            alternate["revision_id"] = obj.get("revision_id", alternate.get("revision_id"))
        field_path = unit.get("field_path", "")
        value = None
        if obj.get("type") in ("text", "formula"):
            value = _object_text(obj)
        elif obj.get("type") == "table" and "cells[" in field_path:
            cell_id = field_path.split("cells[", 1)[1].split("]", 1)[0]
            cell = _find_cell(obj, cell_id)
            if cell:
                value = cell.get("formula") or cell.get("value")
        elif obj.get("type") == "data_chart" and "points[" in field_path:
            point_id = field_path.split("points[", 1)[1].split("]", 1)[0]
            point = _find_point(obj, point_id)
            if point:
                value = "{}".format(point.get("y")) if point.get("y") is not None else ""
        elif obj.get("type") == "structure_diagram" and "edges[" in field_path:
            edge_id = field_path.split("edges[", 1)[1].split("]", 1)[0]
            edge = _find_edge(obj, edge_id)
            if edge:
                value = edge.get("label", "")
        if isinstance(value, str):
            _refresh_unit(unit, value)


def replay_corrections(
    ctx: ExtractionContext,
    drafts: Iterable[Any],
    corrections: Iterable[dict],
    accepted_parse_version_ids: Optional[Iterable[str]] = None,
) -> Tuple[List[dict], List[str]]:
    """Apply only corrections whose base selector and before hash still match."""

    draft_list = list(drafts)
    objects = [obj for draft in draft_list for obj in draft.objects]
    units = [unit for draft in draft_list for unit in draft.search_units]
    output: List[dict] = []
    conflicts: List[str] = []
    for original in corrections:
        correction = deepcopy(original)
        correction_id = str(correction.get("correction_id", "unknown"))
        status = "unmatched"
        selector = correction.get("selector") or {}
        candidates = _selector_object_ids(selector, objects)
        allowed_parse_versions = {ctx.parse_version_id}
        if ctx.parent_parse_version_id:
            allowed_parse_versions.add(ctx.parent_parse_version_id)
        allowed_parse_versions.update(str(value) for value in (accepted_parse_version_ids or []))
        if correction.get("source_version_id") != ctx.source_version_id:
            status = "conflict"
            conflicts.append("{}:source_version_mismatch".format(correction_id))
        elif correction.get("parse_version_id") not in allowed_parse_versions:
            status = "conflict"
            conflicts.append("{}:parse_version_mismatch".format(correction_id))
        elif selector.get("kind") == "material_region" and (
            selector.get("material_id") != ctx.material_id
            or selector.get("source_version_id") != ctx.source_version_id
            or selector.get("parse_version_id") not in allowed_parse_versions
        ):
            status = "conflict"
            conflicts.append("{}:selector_source_mismatch".format(correction_id))
        elif selector.get("kind") not in ("object", "text", "material_region"):
            status = "conflict"
            conflicts.append("{}:selector_unsupported".format(correction_id))
        elif candidates:
            if len(candidates) > 1:
                status = "conflict"
                conflicts.append("{}:ambiguous_selector".format(correction_id))
            else:
                obj = next(o for o in objects if o.get("object_id") == candidates[0])
                scope = str(correction.get("scope", "text")).lower()
                target: Any = _object_text(obj)
                target_kind = "text"
                if "cell" in scope or obj.get("type") == "table":
                    cell_id = None
                    if ":" in scope:
                        cell_id = scope.split(":", 1)[1]
                    target = _find_cell(obj, cell_id)
                    target_kind = "cell"
                    if target is not None:
                        target = target.get("formula") or target.get("value")
                elif "point" in scope or obj.get("type") == "data_chart":
                    point_id = scope.split(":", 1)[1] if ":" in scope else None
                    point = _find_point(obj, point_id)
                    target = point
                    target_kind = "point"
                elif "edge" in scope or obj.get("type") == "structure_diagram":
                    edge_id = scope.split(":", 1)[1] if ":" in scope else None
                    edge = _find_edge(obj, edge_id)
                    target = edge.get("label", "") if edge else None
                    target_kind = "edge"
                if target is None or not _hash_matches(target, correction.get("before_hash", "")):
                    status = "conflict"
                    conflicts.append("{}:before_hash_mismatch".format(correction_id))
                else:
                    replacement = correction.get("replacement")
                    changed = False
                    if target_kind == "text":
                        _new, changed = _apply_to_text(obj, replacement)
                    elif target_kind == "cell":
                        cell = _find_cell(obj, scope.split(":", 1)[1] if ":" in scope else None)
                        if cell is not None and isinstance(replacement, dict):
                            if "formula" in replacement:
                                cell["formula"] = replacement.get("formula")
                                cell["value"] = None
                            if "value" in replacement:
                                cell["value"] = replacement.get("value")
                                cell["formula"] = None
                            cell["status"] = "known" if cell.get("value") or cell.get("formula") else "unknown"
                            changed = True
                        elif cell is not None and isinstance(replacement, str):
                            cell["value"] = replacement
                            cell["formula"] = None
                            cell["status"] = "known"
                            changed = True
                    elif target_kind == "point":
                        point = _find_point(obj, scope.split(":", 1)[1] if ":" in scope else None)
                        if point is not None and isinstance(replacement, dict):
                            for key in ("x", "y", "error_bound", "basis", "value_origin"):
                                if key in replacement:
                                    point[key] = replacement[key]
                            changed = True
                    elif target_kind == "edge":
                        edge = _find_edge(obj, scope.split(":", 1)[1] if ":" in scope else None)
                        if edge is not None and isinstance(replacement, dict):
                            for key in ("label", "directed", "status"):
                                if key in replacement:
                                    edge[key] = replacement[key]
                            changed = True
                    if changed:
                        obj["revision_id"] = ctx.ids.id("object_revision", "{}:correction:{}".format(obj["object_id"], correction_id))
                        _refresh_object_units(units, obj)
                        status = "applied"
                    else:
                        status = "conflict"
                        conflicts.append("{}:unsupported_replacement".format(correction_id))
        else:
            conflicts.append("{}:selector_unmatched".format(correction_id))
        correction["replay_status"] = status
        output.append(correction)
    return output, conflicts
