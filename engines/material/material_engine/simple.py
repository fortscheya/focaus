"""Plain text, Markdown and image fallback inputs."""

from __future__ import annotations

import struct
from pathlib import Path
from typing import Sequence

from .common import ExtractionContext, add_search_unit, base_object, make_text_runs, source_ref
from .errors import EngineError, safe_error
from .models import PageDraft
from .render import PageRenderer


def _image_dimensions(data: bytes, suffix: str):
    if suffix == ".png" and data.startswith(b"\x89PNG") and len(data) >= 24:
        return float(struct.unpack(">I", data[16:20])[0]), float(struct.unpack(">I", data[20:24])[0])
    if suffix == ".gif" and data.startswith((b"GIF87a", b"GIF89a")) and len(data) >= 10:
        return float(struct.unpack("<H", data[6:8])[0]), float(struct.unpack("<H", data[8:10])[0])
    if suffix in (".jpg", ".jpeg") and data.startswith(b"\xff\xd8"):
        index = 2
        while index + 9 < len(data):
            if data[index] != 0xFF:
                index += 1
                continue
            marker = data[index + 1]
            index += 2
            if marker in (0xD8, 0xD9):
                continue
            if index + 2 > len(data):
                break
            length = struct.unpack(">H", data[index : index + 2])[0]
            if marker in range(0xC0, 0xC4) and index + 7 < len(data):
                return float(struct.unpack(">H", data[index + 5 : index + 7])[0]), float(struct.unpack(">H", data[index + 3 : index + 5])[0])
            index += max(2, length)
    return 612.0, 792.0


def extract_text(ctx: ExtractionContext, requested_pages: Sequence[int], renderer: PageRenderer) -> list:
    if any(index != 0 for index in requested_pages):
        raise EngineError("INVALID_ARGUMENT", "text input has only one page", scope="page")
    text = ctx.source_path.read_text(encoding="utf-8", errors="replace")
    page_id = ctx.ids.id("page", "0")
    page = {
        "page_id": page_id,
        "revision_id": ctx.ids.id("page_revision", "0"),
        "source_page_index": 0,
        "display_label": "1",
        "width": 612,
        "height": 792,
        "unit": "pt",
        "rotation": 0,
        "source_transform": [1, 0, 0, 1, 0, 0],
        "preview_resource_id": None,
        "reading_order": [],
        "region_ids": [],
        "status": "complete",
        "errors": [],
    }
    draft = PageDraft(0, page)
    root_region_id = ctx.ids.id("region", "0:root")
    draft.regions.append({"region_id": root_region_id, "page_id": page_id, "bbox": [0, 0, 1, 1], "object_ids": [], "semantic_role": "body", "parent_region_id": None})
    object_region_id = ctx.ids.id("region", "0:object:text")
    draft.regions.append({"region_id": object_region_id, "page_id": page_id, "bbox": [0, 0, 1, 1], "object_ids": [], "semantic_role": "text", "parent_region_id": root_region_id})
    source = source_ref(ctx, page_id, object_region_id, [0, 0, 1, 1])
    block_id, runs = make_text_runs(ctx, 0, "text", text)
    object_value = base_object(ctx, 0, "text", "text", object_region_id, [0, 0, 1, 1], source, extraction="native", editability="native", status="complete", content={"block_id": block_id, "runs": runs, "level": 0})
    draft.objects.append(object_value)
    draft.page["reading_order"].append(object_value["object_id"])
    draft.regions[0]["object_ids"].append(object_value["object_id"])
    draft.regions[1]["object_ids"].append(object_value["object_id"])
    add_search_unit(ctx, draft.search_units, page_index=0, object_value=object_value, text=text, field_path="content.runs", element_ids=[run["element_id"] for run in runs], page_id=page_id, region_id=object_region_id, bbox=[0, 0, 1, 1], equivalence_group_id="eq:{}:text".format(page_id))
    draft.processed_for_search = True
    preview = renderer.render_page(0, 612, 792, text, source_kind="text")
    # render_page returns a placeholder for non-PDF source; keep it as a useful
    # text preview rather than dropping the resource.
    preview_id = ctx.resources.add_bytes("preview/page:0", preview[0], preview[1], "preview", [page_id])
    page["preview_resource_id"] = preview_id
    page["region_ids"] = [region["region_id"] for region in draft.regions]
    return [draft]


def extract_image(ctx: ExtractionContext, requested_pages: Sequence[int], renderer: PageRenderer) -> list:
    if any(index != 0 for index in requested_pages):
        raise EngineError("INVALID_ARGUMENT", "image input has only one page", scope="page")
    data = ctx.source_path.read_bytes()
    width, height = _image_dimensions(data, ctx.source_path.suffix.lower())
    page_id = ctx.ids.id("page", "0")
    page = {
        "page_id": page_id,
        "revision_id": ctx.ids.id("page_revision", "0"),
        "source_page_index": 0,
        "display_label": "1",
        "width": width,
        "height": height,
        "unit": "pt",
        "rotation": 0,
        "source_transform": [1, 0, 0, 1, 0, 0],
        "preview_resource_id": None,
        "reading_order": [],
        "region_ids": [],
        "status": "partial",
        "errors": [safe_error("UNSUPPORTED_CAPABILITY", "image OCR was not configured", retryable=True, scope="page", details={"coverage": "unprocessed", "reason": "ocr_not_configured"})],
    }
    draft = PageDraft(0, page)
    root_region_id = ctx.ids.id("region", "0:root")
    draft.regions.append({"region_id": root_region_id, "page_id": page_id, "bbox": [0, 0, 1, 1], "object_ids": [], "semantic_role": "image", "parent_region_id": None})
    object_id = ctx.ids.id("object", "0:source-image")
    object_region_id = ctx.ids.id("region", "0:object:image")
    media_type = {".png": "image/png", ".jpg": "image/jpeg", ".jpeg": "image/jpeg", ".gif": "image/gif", ".webp": "image/webp"}.get(ctx.source_path.suffix.lower(), "application/octet-stream")
    resource_id = ctx.resources.add_bytes("source/image", data, media_type, "original_page", [page_id, object_id])
    source = source_ref(ctx, page_id, root_region_id, [0, 0, 1, 1])
    object_value = base_object(ctx, 0, "source-image", "image", root_region_id, [0, 0, 1, 1], source, extraction="native", editability="image_only", status="complete", content={"resource_id": resource_id, "alt": "原始图片；未执行 OCR", "crop": [0, 0, 1, 1], "width": width, "height": height})
    draft.objects.append(object_value)
    draft.page["reading_order"].append(object_value["object_id"])
    draft.regions[0]["object_ids"].append(object_value["object_id"])
    page["region_ids"] = [region["region_id"] for region in draft.regions]
    draft.coverage_unprocessed.append({"page_id": page_id, "region_id": root_region_id, "reason": "ocr_not_configured"})
    return [draft]
