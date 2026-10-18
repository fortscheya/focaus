"""Dependency-free PresentationML extraction for the material sidecar."""

from __future__ import annotations

import posixpath
import re
import zipfile
import xml.etree.ElementTree as ET
from pathlib import Path
from typing import Dict, Iterable, List, Optional, Sequence, Tuple

from .common import (
    ExtractionContext,
    add_search_unit,
    base_object,
    clamp_bbox,
    make_text_runs,
    looks_like_formula,
    record_formula_evidence,
    source_ref,
)
from .errors import EngineError, safe_error
from .models import PageDraft
from .render import PageRenderer


P_NS = "http://schemas.openxmlformats.org/presentationml/2006/main"
A_NS = "http://schemas.openxmlformats.org/drawingml/2006/main"
R_NS = "http://schemas.openxmlformats.org/officeDocument/2006/relationships"
REL_NS = "http://schemas.openxmlformats.org/package/2006/relationships"
C_NS = "http://schemas.openxmlformats.org/drawingml/2006/chart"
DGM_NS = "http://schemas.openxmlformats.org/drawingml/2006/diagram"
NS = {"p": P_NS, "a": A_NS, "r": R_NS, "c": C_NS, "dgm": DGM_NS}


def _local(element: ET.Element) -> str:
    return element.tag.rsplit("}", 1)[-1]


def _children(element: ET.Element, local_name: str) -> List[ET.Element]:
    return [child for child in list(element) if _local(child) == local_name]


def _descendants(element: ET.Element, local_name: str) -> List[ET.Element]:
    return [child for child in element.iter() if _local(child) == local_name]


def _first(element: ET.Element, local_name: str) -> Optional[ET.Element]:
    for child in element.iter():
        if _local(child) == local_name:
            return child
    return None


def _attr(element: Optional[ET.Element], local_name: str, namespace: Optional[str] = None) -> Optional[str]:
    if element is None:
        return None
    if namespace:
        value = element.get("{" + namespace + "}" + local_name)
        if value is not None:
            return value
    for key, value in element.attrib.items():
        if key.rsplit("}", 1)[-1] == local_name:
            return value
    return None


def _text_content(element: ET.Element) -> str:
    return "".join(child.text or "" for child in _descendants(element, "t"))


def _slide_number(path: str) -> int:
    match = re.search(r"slide(\d+)\.xml$", path)
    return int(match.group(1)) if match else 0


def _slide_paths(archive: zipfile.ZipFile) -> List[str]:
    return sorted(
        [name for name in archive.namelist() if re.fullmatch(r"ppt/slides/slide\d+\.xml", name)],
        key=_slide_number,
    )


def _rels_for(archive: zipfile.ZipFile, source_path: str) -> Dict[str, Tuple[str, bool]]:
    rels_path = posixpath.join(
        posixpath.dirname(source_path),
        "_rels",
        posixpath.basename(source_path) + ".rels",
    )
    if rels_path not in archive.namelist():
        return {}
    try:
        root = ET.fromstring(archive.read(rels_path))
    except ET.ParseError:
        return {}
    result: Dict[str, Tuple[str, bool]] = {}
    for relation in root:
        rid = relation.get("Id")
        target = relation.get("Target")
        if not rid or not target:
            continue
        external = relation.get("TargetMode") == "External"
        if external:
            result[rid] = (target, True)
        else:
            result[rid] = (posixpath.normpath(posixpath.join(posixpath.dirname(source_path), target)), False)
    return result


def _slide_size(archive: zipfile.ZipFile) -> Tuple[float, float]:
    if "ppt/presentation.xml" not in archive.namelist():
        return 720.0, 540.0
    try:
        root = ET.fromstring(archive.read("ppt/presentation.xml"))
    except ET.ParseError:
        return 720.0, 540.0
    size = _first(root, "sldSz")
    try:
        width = float(_attr(size, "cx") or 9144000) / 12700.0
        height = float(_attr(size, "cy") or 6858000) / 12700.0
        return max(1.0, width), max(1.0, height)
    except ValueError:
        return 720.0, 540.0


def _shape_id(element: ET.Element, fallback: str) -> str:
    c_nv_pr = _first(element, "cNvPr")
    return _attr(c_nv_pr, "id") or fallback


def _shape_name(element: ET.Element) -> str:
    c_nv_pr = _first(element, "cNvPr")
    return _attr(c_nv_pr, "name") or ""


def _shape_description(element: ET.Element) -> str:
    c_nv_pr = _first(element, "cNvPr")
    return _attr(c_nv_pr, "descr") or ""


def _shape_bounds(element: ET.Element, width: float, height: float) -> List[float]:
    xfrm = _first(element, "xfrm")
    off = _first(xfrm, "off") if xfrm is not None else None
    ext = _first(xfrm, "ext") if xfrm is not None else None
    try:
        x = float(_attr(off, "x") or 0) / 12700.0
        y = float(_attr(off, "y") or 0) / 12700.0
        w = float(_attr(ext, "cx") or 0) / 12700.0
        h = float(_attr(ext, "cy") or 0) / 12700.0
        return clamp_bbox([x / width, y / height, w / width, h / height])
    except (TypeError, ValueError, ZeroDivisionError):
        return [0, 0, 1, 1]


def _shape_text(element: ET.Element, page_index: int, key: str, ctx: ExtractionContext) -> Tuple[str, List[dict]]:
    paragraphs = _descendants(element, "p")
    if not paragraphs:
        text = _text_content(element)
        block_id, runs = make_text_runs(ctx, page_index, key, text)
        return text, runs
    runs: List[dict] = []
    lines: List[str] = []
    for paragraph_index, paragraph in enumerate(paragraphs):
        line_parts = []
        for text_index, text_element in enumerate(_descendants(paragraph, "t")):
            text = text_element.text or ""
            line_parts.append(text)
            runs.append(
                {
                    "element_id": ctx.ids.id(
                        "element",
                        "{}:{}:{}".format(page_index, key, len(runs)),
                    ),
                    "text": text,
                }
            )
        if line_parts:
            lines.append("".join(line_parts))
    return "\n".join(lines), runs


def _object_region(
    ctx: ExtractionContext,
    draft: PageDraft,
    page_index: int,
    page_id: str,
    root_region_id: str,
    key: str,
    bounds: Sequence[float],
    object_id: str,
    semantic_role: str,
) -> str:
    region_id = ctx.ids.id("region", "{}:object:{}".format(page_index, key))
    draft.regions.append(
        {
            "region_id": region_id,
            "page_id": page_id,
            "bbox": clamp_bbox(bounds),
            "object_ids": [object_id],
            "semantic_role": semantic_role,
            "parent_region_id": root_region_id,
        }
    )
    return region_id


def _add_shape_text(
    ctx: ExtractionContext,
    draft: PageDraft,
    page_index: int,
    page_id: str,
    root_region_id: str,
    element: ET.Element,
    bounds: Sequence[float],
    shape_key: str,
    z_order: int,
    profile: str,
) -> Optional[dict]:
    text, runs = _shape_text(element, page_index, shape_key, ctx)
    if not text:
        return None
    # Allocate the object before its region so the region can close over the
    # object ID.  The object is then assembled with the exact region ID.
    object_id = ctx.ids.id("object", "{}:{}".format(page_index, shape_key))
    region_id = ctx.ids.id("region", "{}:object:{}".format(page_index, shape_key))
    draft.regions.append(
        {
            "region_id": region_id,
            "page_id": page_id,
            "bbox": clamp_bbox(bounds),
            "object_ids": [object_id],
            "semantic_role": "formula" if profile == "deep" and looks_like_formula(text) else "text",
            "parent_region_id": root_region_id,
        }
    )
    source = source_ref(ctx, page_id, region_id, bounds)
    formula = profile == "deep" and looks_like_formula(text)
    if formula:
        object_value = {
            "object_id": object_id,
            "revision_id": ctx.ids.id("object_revision", "{}:{}".format(page_index, shape_key)),
            "source_refs": [source],
            "region_ids": [region_id],
            "layout": {"bounds": clamp_bbox(bounds), "z_order": z_order, "group_id": None, "constraints": []},
            "extraction": "native",
            "editability": "native",
            "status": "partial",
            "children": [],
            "type": "formula",
            "content": {"latex": text, "display_mode": True, "symbol_binding_ids": []},
        }
        record_formula_evidence(
            ctx,
            page_id=page_id,
            region_id=region_id,
            object_value=object_value,
            latex=text,
        )
        field_path = "content.latex"
        element_ids = [run["element_id"] for run in runs] or [ctx.ids.id("element", shape_key)]
    else:
        block_id = ctx.ids.id("block", "{}:{}".format(page_index, shape_key))
        object_value = {
            "object_id": object_id,
            "revision_id": ctx.ids.id("object_revision", "{}:{}".format(page_index, shape_key)),
            "source_refs": [source],
            "region_ids": [region_id],
            "layout": {"bounds": clamp_bbox(bounds), "z_order": z_order, "group_id": None, "constraints": []},
            "extraction": "native",
            "editability": "native",
            "status": "complete",
            "children": [],
            "type": "text",
            "content": {"block_id": block_id, "runs": runs, "level": 0},
        }
        field_path = "content.runs"
        element_ids = [run["element_id"] for run in runs]
    draft.objects.append(object_value)
    draft.page["reading_order"].append(object_id)
    add_search_unit(
        ctx,
        draft.search_units,
        page_index=page_index,
        object_value=object_value,
        text=text,
        field_path=field_path,
        element_ids=element_ids,
        page_id=page_id,
        region_id=region_id,
        bbox=bounds,
        equivalence_group_id="eq:{}:{}".format(page_id, shape_key),
    )
    return object_value


def _table_from_graphic(
    ctx: ExtractionContext,
    draft: PageDraft,
    page_index: int,
    page_id: str,
    root_region_id: str,
    graphic: ET.Element,
    bounds: Sequence[float],
    shape_key: str,
) -> Optional[dict]:
    table = _first(graphic, "tbl")
    if table is None:
        return None
    rows = _children(table, "tr")
    if not rows:
        return None
    column_count = max((len(_children(row, "tc")) for row in rows), default=0)
    if column_count == 0:
        return None
    object_id = ctx.ids.id("object", "{}:{}".format(page_index, shape_key))
    region_id = ctx.ids.id("region", "{}:object:{}".format(page_index, shape_key))
    source = source_ref(ctx, page_id, region_id, bounds)
    row_ids = [ctx.ids.id("table_row", "{}:{}:{}".format(page_index, shape_key, index)) for index in range(len(rows))]
    column_ids = [ctx.ids.id("table_column", "{}:{}:{}".format(page_index, shape_key, index)) for index in range(column_count)]
    cells: List[dict] = []
    for row_index, row in enumerate(rows):
        for column_index, cell in enumerate(_children(row, "tc")):
            value = _text_content(cell).strip()
            cell_id = ctx.ids.id("table_cell", "{}:{}:{}:{}".format(page_index, shape_key, row_index, column_index))
            is_formula = value.startswith("=")
            cells.append(
                {
                    "cell_id": cell_id,
                    "row_id": row_ids[row_index],
                    "column_id": column_ids[column_index],
                    "rowspan": int(_attr(cell, "rowSpan") or 1),
                    "colspan": int(_attr(cell, "gridSpan") or 1),
                    "value": None if is_formula or not value else value,
                    "unit": None,
                    "formula": value if is_formula else None,
                    "status": "unknown" if is_formula or not value else "known",
                }
            )
    object_value = {
        "object_id": object_id,
        "revision_id": ctx.ids.id("object_revision", "{}:{}".format(page_index, shape_key)),
        "source_refs": [source],
        "region_ids": [region_id],
        "layout": {"bounds": clamp_bbox(bounds), "z_order": len(draft.objects), "group_id": None, "constraints": []},
        "extraction": "native",
        "editability": "native",
        "status": "complete" if all(cell["status"] == "known" for cell in cells) else "partial",
        "children": [],
        "type": "table",
        "content": {"row_ids": row_ids, "column_ids": column_ids, "cells": cells},
    }
    draft.regions.append(
        {
            "region_id": region_id,
            "page_id": page_id,
            "bbox": clamp_bbox(bounds),
            "object_ids": [object_id],
            "semantic_role": "table",
            "parent_region_id": root_region_id,
        }
    )
    draft.objects.append(object_value)
    draft.page["reading_order"].append(object_id)
    for cell in cells:
        value = cell["value"] or cell["formula"] or ""
        if value:
            add_search_unit(
                ctx,
                draft.search_units,
                page_index=page_index,
                object_value=object_value,
                text=value,
                field_path="content.cells[{}].{}".format(cell["cell_id"], "formula" if cell["formula"] else "value"),
                element_ids=[cell["cell_id"]],
                page_id=page_id,
                region_id=region_id,
                bbox=bounds,
                equivalence_group_id="eq:{}:{}".format(page_id, cell["cell_id"]),
            )
    return object_value


def _cache_values(element: Optional[ET.Element]) -> List[Tuple[int, str]]:
    if element is None:
        return []
    cache = _first(element, "numCache")
    if cache is None:
        cache = _first(element, "strCache")
    if cache is None:
        return []
    values = []
    for point in _children(cache, "pt"):
        index = int(_attr(point, "idx") or len(values))
        value_node = _first(point, "v")
        value = (value_node.text or "").strip() if value_node is not None and value_node.text else _text_content(point)
        values.append((index, value))
    return sorted(values)


def _chart_series_name(series: ET.Element) -> str:
    tx = _first(series, "tx")
    if tx is None:
        return ""
    value = _first(tx, "v")
    return (value.text or "").strip() if value is not None and value.text else _text_content(tx).strip()


def _chart_object(
    ctx: ExtractionContext,
    draft: PageDraft,
    archive: zipfile.ZipFile,
    chart_path: Optional[str],
    page_index: int,
    page_id: str,
    root_region_id: str,
    bounds: Sequence[float],
    shape_key: str,
) -> Optional[dict]:
    if not chart_path or chart_path not in archive.namelist():
        return None
    try:
        root = ET.fromstring(archive.read(chart_path))
    except (KeyError, ET.ParseError):
        return None
    region_id = ctx.ids.id("region", "{}:object:{}".format(page_index, shape_key))
    object_id = ctx.ids.id("object", "{}:{}".format(page_index, shape_key))
    source = source_ref(ctx, page_id, region_id, bounds)
    kind = "line"
    for candidate in ("bar", "scatter", "area", "pie", "line"):
        if any(_local(item) == candidate + "Chart" for item in root.iter()):
            kind = candidate
            break
    axes = []
    axis_specs = [("catAx", "category"), ("dateAx", "date"), ("valAx", "linear")]
    for axis_tag, scale in axis_specs:
        for axis in _descendants(root, axis_tag):
            axis_id = ctx.ids.id("chart_axis", "{}:{}:{}".format(page_index, shape_key, len(axes)))
            title = _text_content(_first(axis, "title") or axis).strip() if _first(axis, "title") else ""
            axes.append({"axis_id": axis_id, "label": title, "scale": scale, "unit": None})
    series_values = []
    labels: List[str] = []
    series_elements = _descendants(root, "ser")
    for series_index, series in enumerate(series_elements):
        name = _chart_series_name(series) or "Series {}".format(series_index + 1)
        name_id = ctx.ids.id("chart_label", "{}:{}:name".format(page_index, shape_key, series_index))
        cat = _first(series, "cat")
        val = _first(series, "val")
        categories = _cache_values(cat)
        values = _cache_values(val)
        labels.extend([value for _, value in categories if value])
        value_by_index = dict(values)
        category_by_index = dict(categories)
        points = []
        point_count = max(list(value_by_index.keys()) + list(category_by_index.keys()) + [-1]) + 1
        for point_index in range(point_count):
            raw_x = category_by_index.get(point_index)
            raw_y = value_by_index.get(point_index)
            try:
                x_value = float(raw_x) if raw_x is not None else None
            except ValueError:
                x_value = None
            try:
                y_value = float(raw_y) if raw_y is not None else None
            except (TypeError, ValueError):
                y_value = None
            point_id = ctx.ids.id("chart_point", "{}:{}:{}:{}".format(page_index, shape_key, series_index, point_index))
            points.append(
                {
                    "point_id": point_id,
                    "x": x_value,
                    "y": y_value,
                    "value_origin": "explicit" if y_value is not None else "unknown",
                    "error_bound": None,
                    "basis": "pptx_chart_cache" if y_value is not None else "chart_value_missing",
                }
            )
        series_values.append({"series_id": ctx.ids.id("chart_series", "{}:{}:{}".format(page_index, shape_key, series_index)), "name": name, "points": points})
        add_search_unit(
            ctx,
            draft.search_units,
            page_index=page_index,
            object_value={
                "object_id": object_id,
                "revision_id": ctx.ids.id("object_revision", "{}:{}".format(page_index, shape_key)),
            },
            text=name,
            field_path="content.series[{}].name".format(series_index),
            element_ids=[name_id],
            page_id=page_id,
            region_id=region_id,
            bbox=bounds,
            equivalence_group_id="eq:{}:{}:name".format(page_id, shape_key),
        )
    object_value = {
        "object_id": object_id,
        "revision_id": ctx.ids.id("object_revision", "{}:{}".format(page_index, shape_key)),
        "source_refs": [source],
        "region_ids": [region_id],
        "layout": {"bounds": clamp_bbox(bounds), "z_order": len(draft.objects), "group_id": None, "constraints": []},
        "extraction": "native",
        "editability": "native",
        "status": "complete" if series_values and any(series["points"] for series in series_values) and all(point["value_origin"] == "explicit" for series in series_values for point in series["points"]) else "partial",
        "children": [],
        "type": "data_chart",
        "content": {"chart_kind": kind, "axes": axes, "series": series_values, "legend": [s["name"] for s in series_values], "labels": labels},
    }
    draft.regions.append(
        {
            "region_id": region_id,
            "page_id": page_id,
            "bbox": clamp_bbox(bounds),
            "object_ids": [object_id],
            "semantic_role": "data_chart",
            "parent_region_id": root_region_id,
        }
    )
    draft.objects.append(object_value)
    draft.page["reading_order"].append(object_id)
    for axis_index, axis in enumerate(axes):
        if axis["label"]:
            add_search_unit(
                ctx,
                draft.search_units,
                page_index=page_index,
                object_value=object_value,
                text=axis["label"],
                field_path="content.axes[{}].label".format(axis_index),
                element_ids=[axis["axis_id"]],
                page_id=page_id,
                region_id=region_id,
                bbox=bounds,
                equivalence_group_id="eq:{}:{}:axis:{}".format(page_id, shape_key, axis_index),
            )
    return object_value


def _diagram_from_connectors(
    ctx: ExtractionContext,
    draft: PageDraft,
    page_index: int,
    page_id: str,
    root_region_id: str,
    shape_objects: Dict[str, dict],
    connectors: List[Tuple[str, Optional[str], Optional[str], Sequence[float]]],
) -> Optional[dict]:
    if not connectors:
        return None
    region_id = ctx.ids.id("region", "{}:diagram".format(page_index))
    object_id = ctx.ids.id("object", "{}:diagram".format(page_index))
    object_revision = ctx.ids.id("object_revision", "{}:diagram".format(page_index))
    source = source_ref(ctx, page_id, region_id, [0, 0, 1, 1])
    endpoint_ids = {endpoint for _, start, end, _ in connectors for endpoint in (start, end) if endpoint}
    nodes = []
    for endpoint in sorted(endpoint_ids):
        node_object = shape_objects.get(endpoint)
        label = ""
        position = [0.0, 0.0]
        if node_object:
            if node_object["type"] == "text":
                label = "".join(run["text"] for run in node_object["content"]["runs"])
            elif node_object["type"] == "formula":
                label = node_object["content"]["latex"]
            position = [node_object["layout"]["bounds"][0], node_object["layout"]["bounds"][1]]
        nodes.append({"node_id": ctx.ids.id("diagram_node", "{}:{}".format(page_index, endpoint)), "label": label, "x": position[0], "y": position[1]})
    node_by_endpoint = {endpoint: node["node_id"] for endpoint, node in zip(sorted(endpoint_ids), nodes)}
    edges = []
    for connector_key, start, end, _bounds in connectors:
        edges.append(
            {
                "edge_id": ctx.ids.id("diagram_edge", "{}:{}".format(page_index, connector_key)),
                "from_node_id": node_by_endpoint.get(start, ctx.ids.id("diagram_node", "{}:{}:unknown-start".format(page_index, connector_key))),
                "to_node_id": node_by_endpoint.get(end, ctx.ids.id("diagram_node", "{}:{}:unknown-end".format(page_index, connector_key))),
                "directed": True,
                "label": "",
                "status": "known" if start in node_by_endpoint and end in node_by_endpoint else "unresolved",
            }
        )
    object_value = {
        "object_id": object_id,
        "revision_id": object_revision,
        "source_refs": [source],
        "region_ids": [region_id],
        "layout": {"bounds": [0, 0, 1, 1], "z_order": len(draft.objects), "group_id": None, "constraints": ["connector_endpoints"]},
        "extraction": "native",
        "editability": "native",
        "status": "complete" if all(edge["status"] == "known" for edge in edges) else "partial",
        "children": list(shape_objects.get(endpoint, {}).get("object_id") for endpoint in sorted(endpoint_ids) if endpoint in shape_objects),
        "type": "structure_diagram",
        "content": {"nodes": nodes, "edges": edges},
    }
    draft.regions.append(
        {
            "region_id": region_id,
            "page_id": page_id,
            "bbox": [0, 0, 1, 1],
            "object_ids": [object_id],
            "semantic_role": "structure_diagram",
            "parent_region_id": root_region_id,
        }
    )
    draft.objects.append(object_value)
    draft.page["reading_order"].append(object_id)
    for node in nodes:
        if node["label"]:
            add_search_unit(
                ctx,
                draft.search_units,
                page_index=page_index,
                object_value=object_value,
                text=node["label"],
                field_path="content.nodes[{}].label".format(node["node_id"]),
                element_ids=[node["node_id"]],
                page_id=page_id,
                region_id=region_id,
                bbox=[0, 0, 1, 1],
                equivalence_group_id="eq:{}:diagram:{}:node:{}".format(page_id, object_id, node["node_id"]),
            )
    for edge in edges:
        if edge["label"]:
            add_search_unit(
                ctx,
                draft.search_units,
                page_index=page_index,
                object_value=object_value,
                text=edge["label"],
                field_path="content.edges[{}].label".format(edge["edge_id"]),
                element_ids=[edge["edge_id"]],
                page_id=page_id,
                region_id=region_id,
                bbox=[0, 0, 1, 1],
                equivalence_group_id="eq:{}:diagram:{}:edge:{}".format(page_id, object_id, edge["edge_id"]),
            )
    return object_value


def _image_target(element: ET.Element) -> Optional[str]:
    blip = _first(element, "blip")
    return _attr(blip, "embed", R_NS) if blip is not None else None


def _page_draft(
    ctx: ExtractionContext,
    renderer: PageRenderer,
    archive: zipfile.ZipFile,
    slide_path: str,
    page_index: int,
    width: float,
    height: float,
) -> PageDraft:
    try:
        root = ET.fromstring(archive.read(slide_path))
    except (KeyError, ET.ParseError) as exc:
        page_id = ctx.ids.id("page", str(page_index))
        page = {
            "page_id": page_id,
            "revision_id": ctx.ids.id("page_revision", str(page_index)),
            "source_page_index": page_index,
            "display_label": str(page_index + 1),
            "width": width,
            "height": height,
            "unit": "pt",
            "rotation": 0,
            "source_transform": [1, 0, 0, 1, 0, 0],
            "preview_resource_id": None,
            "reading_order": [],
            "region_ids": [],
            "status": "failed",
            "errors": [safe_error("CORRUPT_ASSET", "PPTX slide XML could not be decoded", scope="page", details={"exception": type(exc).__name__})],
        }
        return PageDraft(page_index, page)
    page_id = ctx.ids.id("page", str(page_index))
    page = {
        "page_id": page_id,
        "revision_id": ctx.ids.id("page_revision", str(page_index)),
        "source_page_index": page_index,
        "display_label": str(page_index + 1),
        "width": width,
        "height": height,
        "unit": "pt",
        "rotation": 0,
        "source_transform": [1, 0, 0, 1, 0, 0],
        "preview_resource_id": None,
        "reading_order": [],
        "region_ids": [],
        "status": "basic_ready",
        "errors": [],
    }
    draft = PageDraft(page_index, page)
    root_region_id = ctx.ids.id("region", "{}:root".format(page_index))
    root_region = {
        "region_id": root_region_id,
        "page_id": page_id,
        "bbox": [0, 0, 1, 1],
        "object_ids": [],
        "semantic_role": "slide",
        "parent_region_id": None,
    }
    draft.regions.append(root_region)
    page["region_ids"].append(root_region_id)
    rels = _rels_for(archive, slide_path)
    slide_text = _text_content(root)
    preview, media_type, render_mode = renderer.render_page(
        page_index, width, height, slide_text, source_kind="pptx"
    )
    preview_id = ctx.resources.add_bytes("preview/page:{}".format(page_index), preview, media_type, "preview", [page_id])
    page["preview_resource_id"] = preview_id
    draft.preview_resource_id = preview_id
    shape_objects: Dict[str, dict] = {}
    connectors: List[Tuple[str, Optional[str], Optional[str], Sequence[float]]] = []
    z_order = 0
    graphic_frames = []
    # The descendants include group members; direct shape ordering is retained
    # by the XML document order, which is the PresentationML z-order.
    for element in root.iter():
        local = _local(element)
        if local == "cxnSp":
            shape_key = "shape:{}".format(_shape_id(element, "connector-{}".format(z_order)))
            bounds = _shape_bounds(element, width, height)
            start = _attr(_first(element, "stCxn"), "id")
            end = _attr(_first(element, "endCxn"), "id")
            connectors.append((shape_key, start, end, bounds))
            z_order += 1
            continue
        if local == "graphicFrame":
            graphic_frames.append((element, z_order))
            z_order += 1
            continue
        if local not in ("sp", "pic"):
            continue
        shape_id = _shape_id(element, "shape-{}".format(z_order))
        shape_key = "shape:{}".format(shape_id)
        bounds = _shape_bounds(element, width, height)
        if local == "pic":
            embed = _image_target(element)
            description = _shape_description(element) or _shape_name(element) or "slide image"
            object_id = ctx.ids.id("object", "{}:{}".format(page_index, shape_key))
            region_id = ctx.ids.id("region", "{}:object:{}".format(page_index, shape_key))
            target = rels.get(embed) if embed else None
            image_bytes = None
            image_type = "application/octet-stream"
            if target and not target[1] and target[0] in archive.namelist():
                image_bytes = archive.read(target[0])
                suffix = Path(target[0]).suffix.lower()
                image_type = {".png": "image/png", ".jpg": "image/jpeg", ".jpeg": "image/jpeg", ".gif": "image/gif", ".webp": "image/webp"}.get(suffix, "application/octet-stream")
            if image_bytes is not None:
                resource_id = ctx.resources.add_bytes("image/{}".format(target[0]), image_bytes, image_type, "image", [object_id])
                source = source_ref(ctx, page_id, region_id, bounds)
                object_value = {
                    "object_id": object_id,
                    "revision_id": ctx.ids.id("object_revision", "{}:{}".format(page_index, shape_key)),
                    "source_refs": [source],
                    "region_ids": [region_id],
                    "layout": {"bounds": clamp_bbox(bounds), "z_order": z_order, "group_id": None, "constraints": []},
                    "extraction": "native",
                    "editability": "native",
                    "status": "complete",
                    "children": [],
                    "type": "image",
                    "content": {"resource_id": resource_id, "alt": description, "crop": [0, 0, 1, 1], "width": width * bounds[2], "height": height * bounds[3]},
                }
                draft.regions.append({"region_id": region_id, "page_id": page_id, "bbox": clamp_bbox(bounds), "object_ids": [object_id], "semantic_role": "image", "parent_region_id": root_region_id})
                draft.objects.append(object_value)
                draft.page["reading_order"].append(object_id)
                shape_objects[shape_id] = object_value
                if description:
                    add_search_unit(ctx, draft.search_units, page_index=page_index, object_value=object_value, text=description, field_path="content.alt", element_ids=[ctx.ids.id("element", "{}:alt".format(shape_key))], page_id=page_id, region_id=region_id, bbox=bounds, equivalence_group_id="eq:{}:{}:alt".format(page_id, shape_key))
            elif target and target[1]:
                page["errors"].append(safe_error("UNSUPPORTED_CAPABILITY", "external image target was not fetched", scope="page", details={"reason": "external_target"}))
            z_order += 1
            continue
        if _first(element, "graphicData") is not None:
            continue
        object_value = _add_shape_text(ctx, draft, page_index, page_id, root_region_id, element, bounds, shape_key, z_order, ctx.profile)
        if object_value is not None:
            shape_objects[shape_id] = object_value
            root_region["object_ids"].append(object_value["object_id"])
        z_order += 1

    for graphic, graphic_z in graphic_frames:
        shape_id = _shape_id(graphic, "graphic-{}".format(graphic_z))
        shape_key = "graphic:{}".format(shape_id)
        bounds = _shape_bounds(graphic, width, height)
        graphic_data = _first(graphic, "graphicData")
        if graphic_data is None:
            continue
        table = _table_from_graphic(ctx, draft, page_index, page_id, root_region_id, graphic_data, bounds, shape_key)
        if table is not None:
            root_region["object_ids"].append(table["object_id"])
            continue
        chart_ref = _first(graphic_data, "chart")
        if chart_ref is not None:
            chart_rid = _attr(chart_ref, "id", R_NS)
            chart_rel = rels.get(chart_rid) if chart_rid else None
            chart = _chart_object(ctx, draft, archive, chart_rel[0] if chart_rel and not chart_rel[1] else None, page_index, page_id, root_region_id, bounds, shape_key)
            if chart is not None:
                root_region["object_ids"].append(chart["object_id"])
            else:
                page["errors"].append(safe_error("UNSUPPORTED_CAPABILITY", "chart structure could not be reconstructed", retryable=True, scope="page", details={"reason": "chart_part_missing"}))
            continue
        if any("diagram" in (uri or "") for uri in [_attr(graphic_data, "uri")]):
            page["errors"].append(safe_error("UNSUPPORTED_CAPABILITY", "SmartArt structure requires reparse", retryable=True, scope="page", details={"reason": "smartart_not_reconstructed"}))

    diagram = _diagram_from_connectors(ctx, draft, page_index, page_id, root_region_id, shape_objects, connectors)
    if diagram is not None:
        root_region["object_ids"].append(diagram["object_id"])
    # Explicitly record media and animation boundaries without retrieving them.
    for target, external in rels.values():
        lower = target.lower()
        if any(lower.endswith(ext) for ext in (".mp4", ".mov", ".avi", ".wmv", ".wav", ".mp3", ".m4a", ".aac")):
            page["errors"].append(safe_error("UNSUPPORTED_CAPABILITY", "embedded audio/video is retained as unsupported metadata", scope="page", details={"reason": "embedded_media"}))
        if external and (lower.startswith("http://") or lower.startswith("https://")):
            page["errors"].append(safe_error("UNSUPPORTED_CAPABILITY", "external link content was not fetched", scope="page", details={"reason": "external_link"}))
    if _first(root, "transition") is not None:
        ctx.add_warning("slide_transition_ignored")
        page["errors"].append(safe_error("UNSUPPORTED_CAPABILITY", "slide transition is not a playback model", scope="page", details={"reason": "transition_ignored"}))
    if not draft.search_units:
        draft.coverage_unprocessed.append({"page_id": page_id, "region_id": root_region_id, "reason": "ocr_not_configured_for_image_only_content"})
    else:
        draft.processed_for_search = True
    if page["errors"] and ctx.profile == "deep":
        page["status"] = "partial"
    elif ctx.profile == "deep" and any(obj["status"] != "complete" for obj in draft.objects):
        page["status"] = "partial"
    elif ctx.profile == "deep":
        page["status"] = "complete"
    else:
        page["status"] = "basic_ready"
    page["region_ids"] = [region["region_id"] for region in draft.regions]
    return draft


def extract_pptx(
    ctx: ExtractionContext,
    requested_pages: Sequence[int],
    renderer: PageRenderer,
) -> List[PageDraft]:
    try:
        archive = zipfile.ZipFile(str(ctx.source_path), "r")
    except (OSError, zipfile.BadZipFile) as exc:
        raise EngineError(
            "CORRUPT_ASSET",
            "source is not a readable PPTX package",
            scope="asset",
            details={"exception": type(exc).__name__},
        )
    with archive:
        slides = _slide_paths(archive)
        if not slides:
            raise EngineError("CORRUPT_ASSET", "PPTX contains no slide XML", scope="asset")
        if not requested_pages:
            requested_pages = list(range(len(slides)))
        if any(index < 0 or index >= len(slides) for index in requested_pages):
            raise EngineError("INVALID_ARGUMENT", "requested PPTX page is out of range", scope="page", details={"page_count": str(len(slides))})
        width, height = _slide_size(archive)
        return [_page_draft(ctx, renderer, archive, slides[index], index, width, height) for index in requested_pages]
