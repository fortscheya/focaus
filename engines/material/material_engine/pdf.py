"""Small, dependency-free PDF text/page extractor.

The sidecar prefers native text streams and delegates visual rendering to the
host's pdftoppm helper.  This parser intentionally does not claim to recover
font glyph maps, arbitrary object geometry, tables, or charts when the PDF
does not expose enough structure.  Those cases remain explicitly partial.
"""

from __future__ import annotations

import re
import zlib
from pathlib import Path
from typing import Dict, Iterable, List, Optional, Sequence, Tuple

from .common import (
    ExtractionContext,
    add_search_unit,
    base_object,
    clamp_bbox,
    layout,
    make_text_runs,
    looks_like_formula,
    page_selector,
    record_formula_evidence,
    region_record,
    source_ref,
)
from .errors import EngineError, safe_error
from .models import PageDraft
from .render import PageRenderer


_OBJECT_RE = re.compile(rb"(\d+)\s+(\d+)\s+obj\b(.*?)endobj", re.S)
_REF_RE = re.compile(rb"(\d+)\s+(\d+)\s+R\b")
_NUMBER_RE = re.compile(rb"[-+]?(?:\d+\.\d*|\.\d+|\d+)")


def _parse_objects(data: bytes) -> Dict[int, bytes]:
    return {int(match.group(1)): match.group(3) for match in _OBJECT_RE.finditer(data)}


def _references(value: bytes) -> List[int]:
    return [int(match.group(1)) for match in _REF_RE.finditer(value)]


def _array_numbers(value: bytes, key: bytes) -> Optional[List[float]]:
    match = re.search(re.escape(key) + rb"\s*\[([^]]+)\]", value, re.S)
    if not match:
        return None
    return [float(item) for item in _NUMBER_RE.findall(match.group(1))]


def _inherited_array(objects: Dict[int, bytes], page_number: int, key: bytes) -> Optional[List[float]]:
    """Resolve MediaBox-like values inherited from the Pages tree."""
    visited = set()
    current = page_number
    while current in objects and current not in visited:
        visited.add(current)
        value = _array_numbers(objects[current], key)
        if value is not None:
            return value
        parents = _references(re.search(rb"/Parent\s*(.*?)(?:/\w+\s|>>|$)", objects[current], re.S).group(1)) if re.search(rb"/Parent\s*(.*?)(?:/\w+\s|>>|$)", objects[current], re.S) else []
        if not parents:
            break
        current = parents[0]
    return None


def _inherited_scalar(objects: Dict[int, bytes], page_number: int, key: bytes) -> Optional[float]:
    visited = set()
    current = page_number
    while current in objects and current not in visited:
        visited.add(current)
        match = re.search(re.escape(key) + rb"\s+([-+]?\d+(?:\.\d+)?)", objects[current])
        if match:
            return float(match.group(1))
        parents = _references(re.search(rb"/Parent\s*(.*?)(?:/\w+\s|>>|$)", objects[current], re.S).group(1)) if re.search(rb"/Parent\s*(.*?)(?:/\w+\s|>>|$)", objects[current], re.S) else []
        if not parents:
            break
        current = parents[0]
    return None


def _decode_pdf_string(value: bytes) -> str:
    if value.startswith(b"<") and value.endswith(b">"):
        raw = re.sub(rb"\s+", b"", value[1:-1])
        if len(raw) % 2:
            raw += b"0"
        try:
            decoded = bytes.fromhex(raw.decode("ascii"))
        except (ValueError, UnicodeDecodeError):
            return ""
    else:
        raw = value[1:-1]
        output = bytearray()
        index = 0
        escapes = {ord("n"): 10, ord("r"): 13, ord("t"): 9, ord("b"): 8, ord("f"): 12}
        while index < len(raw):
            char = raw[index]
            if char != 92:
                output.append(char)
                index += 1
                continue
            index += 1
            if index >= len(raw):
                break
            char = raw[index]
            if char in escapes:
                output.append(escapes[char])
                index += 1
            elif char in (10, 13):
                if char == 13 and index + 1 < len(raw) and raw[index + 1] == 10:
                    index += 1
                index += 1
            elif 48 <= char <= 55:
                digits = [char]
                index += 1
                while index < len(raw) and len(digits) < 3 and 48 <= raw[index] <= 55:
                    digits.append(raw[index])
                    index += 1
                output.append(int(bytes(digits), 8))
            else:
                output.append(char)
                index += 1
        decoded = bytes(output)
    if decoded.startswith(b"\xfe\xff"):
        return decoded[2:].decode("utf-16-be", "replace")
    try:
        return decoded.decode("utf-8")
    except UnicodeDecodeError:
        return decoded.decode("latin-1", "replace")


def _balanced_literal(data: bytes, start: int) -> Tuple[bytes, int]:
    depth = 0
    index = start
    escaped = False
    while index < len(data):
        char = data[index]
        if escaped:
            escaped = False
            index += 1
            continue
        if char == 92:
            escaped = True
            index += 1
            continue
        if char == 40:
            depth += 1
        elif char == 41:
            depth -= 1
            if depth == 0:
                return data[start : index + 1], index + 1
        index += 1
    return data[start:], len(data)


def _tokens(data: bytes) -> List[Tuple[str, object]]:
    tokens: List[Tuple[str, object]] = []
    index = 0
    while index < len(data):
        char = data[index]
        if char in b" \t\r\n\f\x00":
            index += 1
            continue
        if char == 37:  # % comment
            end = data.find(b"\n", index)
            index = len(data) if end < 0 else end + 1
            continue
        if char == 40:  # literal string
            value, index = _balanced_literal(data, index)
            tokens.append(("string", value))
            continue
        if char == 60 and not data[index : index + 2] == b"<<":  # hex string
            end = data.find(b">", index + 1)
            end = len(data) - 1 if end < 0 else end
            tokens.append(("string", data[index : end + 1]))
            index = end + 1
            continue
        if char == 91:
            tokens.append(("open_array", "["))
            index += 1
            continue
        if char == 93:
            tokens.append(("close_array", "]"))
            index += 1
            continue
        end = index + 1
        while end < len(data) and data[end] not in b" \t\r\n\f\x00[]()<>%":
            end += 1
        tokens.append(("word", data[index:end].decode("latin-1", "replace")))
        index = end
    return tokens


def _extract_text(stream: bytes) -> str:
    tokens = _tokens(stream)
    output: List[str] = []
    array_values: List[bytes] = []
    in_array = False
    pending_break = False
    for kind, value in tokens:
        if kind == "open_array":
            in_array = True
            array_values = []
        elif kind == "close_array":
            in_array = False
        elif kind == "string":
            if in_array:
                array_values.append(value)  # type: ignore[arg-type]
            else:
                # Remember the most recent string.  It is consumed by Tj/'/".
                array_values = [value]  # type: ignore[list-item]
        elif kind == "word":
            word = value
            if word in ("Tj", "'", '"'):
                if array_values:
                    text = _decode_pdf_string(array_values[-1])
                    if text:
                        if pending_break and output:
                            output.append("\n")
                        output.append(text)
                    array_values = []
                pending_break = word in ("'", '"')
            elif word == "TJ":
                if array_values:
                    text = "".join(_decode_pdf_string(item) for item in array_values if isinstance(item, bytes))
                    if text:
                        if pending_break and output:
                            output.append("\n")
                        output.append(text)
                    array_values = []
                pending_break = False
            elif word in ("T*", "Td", "TD"):
                pending_break = True
            elif not in_array and word not in ("BT", "ET", "Tf", "Tm", "cm", "Do", "q", "Q"):
                # Do not consume numeric operands as text.  Keeping the last
                # string until Tj handles the common PDF content order.
                pass
    text = "".join(output)
    text = text.replace("\r\n", "\n").replace("\r", "\n")
    return text.strip()


def _stream_bytes(body: bytes) -> bytes:
    marker = body.find(b"stream")
    if marker < 0:
        return b""
    start = marker + len(b"stream")
    if body[start : start + 2] == b"\r\n":
        start += 2
    elif body[start : start + 1] in (b"\r", b"\n"):
        start += 1
    end = body.find(b"endstream", start)
    raw = body[start:] if end < 0 else body[start:end]
    dictionary = body[:marker]
    if b"/FlateDecode" in dictionary or b"/Fl" in dictionary:
        try:
            return zlib.decompress(raw.strip())
        except zlib.error:
            # Some producers include a byte or newline after the declared
            # stream.  Try a small tolerant search before reporting no text.
            for offset in range(1, min(16, len(raw))):
                try:
                    return zlib.decompress(raw[offset:].strip())
                except zlib.error:
                    continue
            return b""
    return raw


def _page_order(objects: Dict[int, bytes]) -> List[int]:
    roots = [number for number, body in objects.items() if re.search(rb"/Type\s*/Pages(?:\s|/|$)", body)]
    visited = set()
    ordered: List[int] = []

    def visit(number: int) -> None:
        if number in visited or number not in objects:
            return
        visited.add(number)
        body = objects[number]
        if re.search(rb"/Type\s*/Page(?:\s|/|$)", body):
            ordered.append(number)
            return
        kids_match = re.search(rb"/Kids\s*\[([^]]*)\]", body, re.S)
        if kids_match:
            for child in _references(kids_match.group(1)):
                visit(child)

    for root in roots:
        visit(root)
    if ordered:
        return ordered
    return [number for number in sorted(objects) if re.search(rb"/Type\s*/Page(?:\s|/|$)", objects[number])]


def _page_content(objects: Dict[int, bytes], body: bytes) -> bytes:
    content_ids = _references(re.search(rb"/Contents\s*(.*?)(?:/\w+\s|>>|$)", body, re.S).group(1)) if re.search(rb"/Contents\s*(.*?)(?:/\w+\s|>>|$)", body, re.S) else []
    streams = []
    for number in content_ids:
        if number in objects:
            streams.append(_stream_bytes(objects[number]))
    return b"\n".join(streams)


def _table_lines(text: str) -> Optional[List[List[str]]]:
    lines = [line.strip() for line in text.splitlines() if line.strip()]
    if len(lines) < 2:
        return None
    rows = [re.split(r"\s{2,}|\t+|\s*\|\s*", line.strip("| ")) for line in lines]
    width = len(rows[0])
    if width < 2 or any(len(row) != width for row in rows):
        return None
    return rows


def _make_table(
    ctx: ExtractionContext,
    draft: PageDraft,
    page_index: int,
    page_id: str,
    region_id: str,
    text: str,
    rows: List[List[str]],
    bounds: Sequence[float],
) -> dict:
    table_region_id = ctx.ids.id("region", "{}:table:0".format(page_index))
    table_object_id = ctx.ids.id("object", "{}:table:0".format(page_index))
    table_source = source_ref(ctx, page_id, table_region_id, bounds)
    row_ids = [ctx.ids.id("table_row", "{}:{}".format(page_index, index)) for index in range(len(rows))]
    column_ids = [ctx.ids.id("table_column", "{}:{}".format(page_index, index)) for index in range(len(rows[0]))]
    cells = []
    for row_index, row in enumerate(rows):
        for column_index, value in enumerate(row):
            cell_id = ctx.ids.id("table_cell", "{}:{}:{}".format(page_index, row_index, column_index))
            cells.append(
                {
                    "cell_id": cell_id,
                    "row_id": row_ids[row_index],
                    "column_id": column_ids[column_index],
                    "rowspan": 1,
                    "colspan": 1,
                    "value": value or None,
                    "unit": None,
                    "formula": None,
                    "status": "known" if value else "unknown",
                }
            )
    table = base_object(
        ctx,
        page_index,
        "table:0",
        "table",
        table_region_id,
        bounds,
        table_source,
        extraction="text",
        editability="native",
        status="complete" if all(cell["status"] == "known" for cell in cells) else "partial",
        content={"row_ids": row_ids, "column_ids": column_ids, "cells": cells},
    )
    # Keep the page region relationship closed and expose a precise table region.
    draft.regions.append(
        {
            "region_id": table_region_id,
            "page_id": page_id,
            "bbox": clamp_bbox(bounds),
            "object_ids": [table["object_id"]],
            "semantic_role": "table",
            "parent_region_id": region_id,
        }
    )
    for cell in cells:
        value = cell["value"] or ""
        if value:
            add_search_unit(
                ctx,
                draft.search_units,
                page_index=page_index,
                object_value=table,
                text=value,
                field_path="content.cells[{}].value".format(cell["cell_id"]),
                element_ids=[cell["cell_id"]],
                page_id=page_id,
                region_id=table_region_id,
                bbox=bounds,
                equivalence_group_id="eq:{}:{}".format(page_id, cell["cell_id"]),
            )
    draft.objects.append(table)
    draft.page["reading_order"].append(table["object_id"])
    return table


def _page_draft(
    ctx: ExtractionContext,
    renderer: PageRenderer,
    objects: Dict[int, bytes],
    page_object_number: int,
    page_index: int,
    *,
    profile: str,
) -> PageDraft:
    body = objects[page_object_number]
    media_box = _inherited_array(objects, page_object_number, b"/MediaBox") or [0.0, 0.0, 612.0, 792.0]
    width = max(1.0, media_box[2] - media_box[0]) if len(media_box) >= 4 else 612.0
    height = max(1.0, media_box[3] - media_box[1]) if len(media_box) >= 4 else 792.0
    rotation_value = _inherited_scalar(objects, page_object_number, b"/Rotate") or 0.0
    rotation = int(rotation_value) % 360
    if rotation not in (0, 90, 180, 270):
        rotation = 0
    if rotation in (90, 270):
        width, height = height, width
    page_id = ctx.ids.id("page", str(page_index))
    page = {
        "page_id": page_id,
        "revision_id": ctx.ids.id("page_revision", str(page_index)),
        "source_page_index": page_index,
        "display_label": str(page_index + 1),
        "width": width,
        "height": height,
        "unit": "pt",
        "rotation": rotation,
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
        "semantic_role": "body",
        "parent_region_id": None,
    }
    draft.regions.append(root_region)
    page["region_ids"].append(root_region_id)
    try:
        stream = _page_content(objects, body)
        text = _extract_text(stream)
    except Exception as exc:  # pragma: no cover - defensive for malformed PDFs
        text = ""
        page["errors"].append(
            safe_error(
                "CORRUPT_ASSET",
                "PDF text stream could not be decoded",
                scope="page",
                details={"exception": type(exc).__name__},
            )
        )

    preview, media_type, render_mode = renderer.render_page(
        page_index, width, height, text, source_kind="pdf"
    )
    preview_id = ctx.resources.add_bytes(
        "preview/page:{}".format(page_index),
        preview,
        media_type,
        "preview",
        [page_id],
    )
    page["preview_resource_id"] = preview_id
    draft.preview_resource_id = preview_id

    if text:
        table = _table_lines(text) if profile == "deep" else None
        if table:
            table_obj = _make_table(ctx, draft, page_index, page_id, root_region_id, text, table, [0, 0, 1, 1])
            root_region["object_ids"].append(table_obj["object_id"])
            draft.processed_for_search = True
        else:
            key = "text:0"
            object_region_id = ctx.ids.id("region", "{}:object:0".format(page_index))
            object_region = {
                "region_id": object_region_id,
                "page_id": page_id,
                "bbox": [0, 0, 1, 1],
                "object_ids": [],
                "semantic_role": "text",
                "parent_region_id": root_region_id,
            }
            draft.regions.append(object_region)
            page["region_ids"].append(object_region_id)
            object_source = source_ref(ctx, page_id, object_region_id, [0, 0, 1, 1])
            formula = profile == "deep" and looks_like_formula(text)
            element_id = ctx.ids.id("element", "{}:text:0".format(page_index))
            if formula:
                object_value = base_object(
                    ctx,
                    page_index,
                    key,
                    "formula",
                    object_region_id,
                    [0, 0, 1, 1],
                    object_source,
                    extraction="text",
                    editability="native",
                    status="partial",
                    content={"latex": text, "display_mode": True, "symbol_binding_ids": []},
                )
                record_formula_evidence(
                    ctx,
                    page_id=page_id,
                    region_id=object_region_id,
                    object_value=object_value,
                    latex=text,
                )
                field_path = "content.latex"
                element_ids = [element_id]
            else:
                block_id, runs = make_text_runs(ctx, page_index, key, text)
                object_value = base_object(
                    ctx,
                    page_index,
                    key,
                    "text",
                    object_region_id,
                    [0, 0, 1, 1],
                    object_source,
                    extraction="text",
                    editability="native",
                    status="complete",
                    content={"block_id": block_id, "runs": runs, "level": 0},
                )
                field_path = "content.runs"
                element_ids = [run["element_id"] for run in runs]
            object_region["object_ids"].append(object_value["object_id"])
            root_region["object_ids"].append(object_value["object_id"])
            draft.objects.append(object_value)
            page["reading_order"].append(object_value["object_id"])
            add_search_unit(
                ctx,
                draft.search_units,
                page_index=page_index,
                object_value=object_value,
                text=text,
                field_path=field_path,
                element_ids=element_ids,
                page_id=page_id,
                region_id=object_region_id,
                bbox=[0, 0, 1, 1],
                equivalence_group_id="eq:{}:text".format(page_id),
            )
            draft.processed_for_search = True
        if profile == "deep" and any(obj["status"] != "complete" for obj in draft.objects):
            page["status"] = "partial"
        else:
            page["status"] = "complete" if profile == "deep" else "basic_ready"
    else:
        image_source = source_ref(ctx, page_id, root_region_id, [0, 0, 1, 1])
        image_object = base_object(
            ctx,
            page_index,
            "page-image",
            "image",
            root_region_id,
            [0, 0, 1, 1],
            image_source,
            extraction="native",
            editability="image_only",
            status="partial",
            content={
                "resource_id": preview_id,
                "alt": "原页回退：未检测到可提取文字层",
                "crop": [0, 0, 1, 1],
                "width": width,
                "height": height,
            },
        )
        root_region["object_ids"].append(image_object["object_id"])
        draft.objects.append(image_object)
        page["reading_order"].append(image_object["object_id"])
        page["status"] = "partial" if render_mode else "failed"
        page["errors"].append(
            safe_error(
                "UNSUPPORTED_CAPABILITY",
                "page has no native text layer; OCR was not configured",
                retryable=True,
                scope="page",
                details={"coverage": "unprocessed", "reason": "ocr_not_configured"},
            )
        )
        draft.coverage_unprocessed.append(
            {"page_id": page_id, "region_id": root_region_id, "reason": "ocr_not_configured"}
        )
    page["region_ids"] = [region["region_id"] for region in draft.regions]
    return draft


def extract_pdf(
    ctx: ExtractionContext,
    requested_pages: Sequence[int],
    renderer: PageRenderer,
) -> List[PageDraft]:
    try:
        data = ctx.source_path.read_bytes()
    except OSError as exc:
        raise EngineError(
            "SOURCE_UNAVAILABLE",
            "cannot read PDF source",
            retryable=True,
            scope="asset",
            details={"exception": type(exc).__name__},
        )
    if not data.startswith(b"%PDF-"):
        raise EngineError("CORRUPT_ASSET", "source does not have a PDF header", scope="asset")
    objects = _parse_objects(data)
    page_numbers = _page_order(objects)
    if not page_numbers:
        raise EngineError("CORRUPT_ASSET", "PDF contains no page objects", scope="asset")
    if not requested_pages:
        requested_pages = list(range(len(page_numbers)))
    if any(index < 0 or index >= len(page_numbers) for index in requested_pages):
        raise EngineError(
            "INVALID_ARGUMENT",
            "requested PDF page is out of range",
            scope="page",
            details={"page_count": str(len(page_numbers))},
        )
    drafts = []
    for index in requested_pages:
        drafts.append(
            _page_draft(
                ctx,
                renderer,
                objects,
                page_numbers[index],
                index,
                profile=ctx.profile,
            )
        )
    return drafts
