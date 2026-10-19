import json
import os
import queue
import shutil
import subprocess
import sys
import tempfile
import threading
import time
import uuid
import zipfile
from pathlib import Path
from unittest import TestCase

from material_engine.common import IdentityAllocator, ResourceStore, content_hash, safe_relative_path, text_hash
from material_engine.engine import MaterialEngine, _draft_from_manifest
from material_engine.errors import EngineError
from material_engine.manifest import validate_manifest
from material_engine.protocol import EngineProtocol


def make_pdf(path: Path, texts):
    page_objects = []
    object_number = 3
    content_number = 3 + len(texts) * 2
    for index, text in enumerate(texts):
        page_number = object_number
        stream_number = page_number + 1
        content = "BT /F1 12 Tf 72 720 Td ({}) Tj ET".format(text.replace("\\", "\\\\").replace("(", "\\(").replace(")", "\\)"))
        page_objects.append((page_number, stream_number, content.encode("latin-1", "replace")))
        object_number += 2
    parts = [b"%PDF-1.4\n", b"1 0 obj << /Type /Catalog /Pages 2 0 R >> endobj\n"]
    kids = " ".join("{} 0 R".format(page_number) for page_number, _, _ in page_objects)
    parts.append("2 0 obj << /Type /Pages /Kids [{}] /Count {} >> endobj\n".format(kids, len(page_objects)).encode())
    for page_number, stream_number, content in page_objects:
        parts.append("{} 0 obj << /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents {} 0 R >> endobj\n".format(page_number, stream_number).encode())
        parts.append("{} 0 obj << /Length {} >> stream\n".format(stream_number, len(content)).encode() + content + b"\nendstream endobj\n")
    parts.append(b"%%EOF\n")
    path.write_bytes(b"".join(parts))


def make_pptx(path: Path):
    slide = '''<p:sld xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"><p:cSld><p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:sp><p:nvSpPr><p:cNvPr id="2" name="Text"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr><p:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="5000000" cy="1000000"/></a:xfrm></p:spPr><p:txBody><a:bodyPr/><a:lstStyle/><a:p><a:r><a:t>alpha β</a:t></a:r></a:p></p:txBody></p:sp></p:spTree></p:cSld></p:sld>'''
    presentation = '<p:presentation xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"><p:sldSz cx="9144000" cy="6858000"/></p:presentation>'
    with zipfile.ZipFile(path, "w") as archive:
        archive.writestr("ppt/presentation.xml", presentation)
        archive.writestr("ppt/slides/slide1.xml", slide)


def make_rich_pptx(path: Path):
    slide = '''<p:sld xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><p:cSld><p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:sp><p:nvSpPr><p:cNvPr id="2" name="A"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr><p:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="2000000" cy="1000000"/></a:xfrm></p:spPr><p:txBody><a:bodyPr/><a:lstStyle/><a:p><a:r><a:t>A</a:t></a:r></a:p></p:txBody></p:sp><p:sp><p:nvSpPr><p:cNvPr id="3" name="B"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr><p:spPr><a:xfrm><a:off x="4000000" y="0"/><a:ext cx="2000000" cy="1000000"/></a:xfrm></p:spPr><p:txBody><a:bodyPr/><a:lstStyle/><a:p><a:r><a:t>B</a:t></a:r></a:p></p:txBody></p:sp><p:cxnSp><p:nvCxnSpPr><p:cNvPr id="4" name="Arrow"/><p:cNvCxnSpPr/><p:nvPr/></p:nvCxnSpPr><p:spPr><a:xfrm><a:off x="2000000" y="500000"/><a:ext cx="2000000" cy="0"/></a:xfrm><a:prstGeom prst="line"><a:avLst/></a:prstGeom><p:stCxn id="2" idx="0"/><p:endCxn id="3" idx="0"/></p:spPr></p:cxnSp><p:graphicFrame><p:nvGraphicFramePr><p:cNvPr id="5" name="Chart"/><p:cNvGraphicFramePr/><p:nvPr/></p:nvGraphicFramePr><p:xfrm><a:off x="0" y="2000000"/><a:ext cx="5000000" cy="2500000"/></p:xfrm><a:graphic><a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/chart"><c:chart xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" r:id="rId1"/></a:graphicData></a:graphic></p:graphicFrame></p:spTree></p:cSld></p:sld>'''
    relationships = '''<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/chart" Target="../charts/chart1.xml"/></Relationships>'''
    chart = '''<c:chartSpace xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart"><c:chart><c:plotArea><c:lineChart><c:axId val="1"/><c:axId val="2"/><c:ser><c:idx val="0"/><c:order val="0"/><c:tx><c:v>Series A</c:v></c:tx><c:cat><c:strRef><c:strCache><c:ptCount val="2"/><c:pt idx="0"><c:v>Jan</c:v></c:pt><c:pt idx="1"><c:v>Feb</c:v></c:pt></c:strCache></c:strRef></c:cat><c:val><c:numRef><c:numCache><c:ptCount val="2"/><c:pt idx="0"><c:v>1</c:v></c:pt><c:pt idx="1"><c:v>2</c:v></c:pt></c:numCache></c:numRef></c:val></c:ser></c:lineChart><c:catAx><c:axId val="1"/></c:catAx><c:valAx><c:axId val="2"/></c:valAx></c:plotArea></c:chart></c:chartSpace>'''
    presentation = '<p:presentation xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"><p:sldSz cx="9144000" cy="6858000"/></p:presentation>'
    with zipfile.ZipFile(path, "w") as archive:
        archive.writestr("ppt/presentation.xml", presentation)
        archive.writestr("ppt/slides/slide1.xml", slide)
        archive.writestr("ppt/slides/_rels/slide1.xml.rels", relationships)
        archive.writestr("ppt/charts/chart1.xml", chart)


def material_selector_parse_versions(value):
    versions = []
    if isinstance(value, dict):
        if value.get("kind") == "material_region" and "parse_version_id" in value:
            versions.append(value["parse_version_id"])
        for nested in value.values():
            versions.extend(material_selector_parse_versions(nested))
    elif isinstance(value, list):
        for nested in value:
            versions.extend(material_selector_parse_versions(nested))
    return versions


class MaterialEngineTests(TestCase):
    def setUp(self):
        self.root = Path(tempfile.mkdtemp(prefix="focaus-material-test-"))
        self.engine = MaterialEngine()

    def tearDown(self):
        shutil.rmtree(self.root, ignore_errors=True)

    def payload(self, source: Path, profile="light", pages=None, stage_name="stage"):
        values = [str(uuid.uuid4()) for _ in range(4)]
        return values, {
            "material_id": values[0],
            "source_version_id": values[1],
            "parse_version_id": values[2],
            "source_handle": str(source),
            "staging_handle": str(self.root / stage_name),
            "identity_pool": {},
            "profile": profile,
            "pages": [] if pages is None else pages,
            "corrections": [],
            "budget_ms": 120000,
        }

    def test_pdf_manifest_has_search_anchors_and_resource_closure(self):
        source = self.root / "sample.pdf"
        make_pdf(source, ["Hello 学习"])
        values, payload = self.payload(source)
        result = self.engine.convert(payload, task_id=values[3])
        manifest = result["manifest"]
        self.assertEqual(manifest["format_version"], "2.0.0")
        self.assertEqual(manifest["completion"], "complete")
        self.assertEqual(manifest["search_coverage"]["state"], "complete")
        self.assertEqual(len(manifest["search_units"]), 1)
        unit = manifest["search_units"][0]
        self.assertEqual(unit["anchor"]["selector"]["kind"], "object")
        self.assertEqual(unit["alternate_anchors"][0]["selector"]["kind"], "material_region")
        material_selector = unit["alternate_anchors"][0]["selector"]
        self.assertEqual(material_selector["material_id"], manifest["material_id"])
        self.assertEqual(material_selector["source_version_id"], manifest["source_version_id"])
        self.assertEqual(material_selector["parse_version_id"], manifest["parse_version_id"])
        self.assertEqual(unit["content_hash"], text_hash(unit["display_text"]))
        for resource in manifest["resources"]:
            self.assertTrue((self.root / "stage" / resource["relative_path"]).is_file())

    def test_correction_replay_updates_search_text_without_mutating_source(self):
        source = self.root / "sample.pdf"
        make_pdf(source, ["before"])
        values, payload = self.payload(source, stage_name="correction")
        first = self.engine.convert(payload, task_id=values[3])
        object_value = first["manifest"]["objects"][0]
        old_snapshot_bytes = Path(first["manifest_path"]).read_bytes()
        correction = {
            "correction_id": str(uuid.uuid4()),
            "source_version_id": values[1],
            "parse_version_id": values[2],
            "selector": {"kind": "object", "object_id": object_value["object_id"], "element_ids": [object_value["content"]["runs"][0]["element_id"]]},
            "before_hash": content_hash("before"),
            "replacement": "after",
            "scope": "text",
            "replay_status": "applied",
        }
        values2, payload2 = self.payload(source, stage_name="correction-reparse")
        payload2["material_id"] = values[0]
        payload2["source_version_id"] = values[1]
        payload2["parse_version_id"] = str(uuid.uuid4())
        payload2["parent_parse_version_id"] = values[2]
        payload2["parent_manifest_handle"] = str(self.root / "correction" / "snapshots" / (values[2] + ".json"))
        payload2["corrections"] = [correction]
        result = self.engine.convert(payload2, task_id=values2[3])
        self.assertEqual(result["manifest"]["objects"][0]["content"]["runs"][0]["text"], "after")
        self.assertEqual(result["manifest"]["search_units"][0]["display_text"], "after")
        self.assertEqual(result["manifest"]["corrections_applied"][0]["replay_status"], "applied")
        self.assertEqual(result["manifest"]["parent_parse_version_id"], values[2])
        self.assertEqual(source.read_bytes().count(b"before"), 1)
        self.assertEqual(Path(first["manifest_path"]).read_bytes(), old_snapshot_bytes)

    def test_stale_material_correction_is_retained_as_conflict_without_mutating_parent(self):
        source = self.root / "stale-correction.pdf"
        make_pdf(source, ["stable"])
        values, payload = self.payload(source, stage_name="stale-correction")
        first = self.engine.convert(payload, task_id=values[3])
        parent_bytes = Path(first["manifest_path"]).read_bytes()
        object_value = first["manifest"]["objects"][0]
        region_id = object_value["region_ids"][0]
        stale_correction = {
            "correction_id": str(uuid.uuid4()),
            "source_version_id": values[1],
            "parse_version_id": values[2],
            "selector": {
                "kind": "material_region",
                "material_id": str(uuid.uuid4()),
                "source_version_id": values[1],
                "parse_version_id": values[2],
                "page_id": first["manifest"]["pages"][0]["page_id"],
                "region_id": region_id,
                "bbox": [0, 0, 1, 1],
                "coordinate_space": "rotated_page_normalized",
            },
            "before_hash": content_hash("stable"),
            "replacement": "changed",
            "scope": "text",
            "replay_status": "applied",
        }
        values2, reparse = self.payload(source, stage_name="stale-correction-reparse")
        reparse.update(
            {
                "material_id": values[0],
                "source_version_id": values[1],
                "parse_version_id": str(uuid.uuid4()),
                "parent_parse_version_id": values[2],
                "parent_manifest_handle": str(self.root / "stale-correction" / "snapshots" / (values[2] + ".json")),
                "corrections": [stale_correction],
            }
        )
        result = self.engine.convert(reparse, task_id=values2[3])
        self.assertEqual(result["manifest"]["objects"][0]["content"]["runs"][0]["text"], "stable")
        self.assertEqual(result["manifest"]["corrections_applied"][0]["replay_status"], "conflict")
        self.assertIn(stale_correction["correction_id"] + ":selector_source_mismatch", result["manifest"]["unresolved_conflicts"])
        self.assertEqual(Path(first["manifest_path"]).read_bytes(), parent_bytes)

    def test_each_page_publication_is_an_immutable_snapshot_with_existing_parent(self):
        source = self.root / "publication-chain.pdf"
        make_pdf(source, ["first", "second"])
        values, payload = self.payload(source, pages=[0, 1], stage_name="publication-chain")
        page_results = []
        result = self.engine.convert(payload, task_id=values[3], on_page=page_results.append)
        manifest = result["manifest"]
        published = result["published_parse_versions"]
        self.assertEqual(len(published), 3)
        self.assertEqual(published[-1], values[2])
        self.assertEqual(len(set(published)), 3)
        self.assertEqual([item["manifest"]["parse_version_id"] for item in page_results], published[:2])
        self.assertEqual([len(item["manifest"]["pages"]) for item in page_results], [1, 2])
        self.assertIsNone(manifest["parent_parse_version_id"])
        requested_page_ids = [page["page_id"] for page in manifest["pages"]]
        self.assertEqual(page_results[0]["manifest"]["search_coverage"]["requested_pages"], requested_page_ids)
        self.assertEqual(page_results[1]["manifest"]["search_coverage"]["requested_pages"], requested_page_ids)
        self.assertEqual(page_results[0]["manifest"]["search_coverage"]["processed_pages"], [requested_page_ids[0]])
        self.assertEqual(page_results[1]["manifest"]["search_coverage"]["processed_pages"], requested_page_ids)
        self.assertEqual(
            [area["page_id"] for area in page_results[0]["manifest"]["search_coverage"]["unprocessed"]],
            [requested_page_ids[1]],
        )
        self.assertEqual(page_results[1]["manifest"]["search_coverage"]["unprocessed"], [])
        for index, page_result in enumerate(page_results):
            publication_id = published[index]
            self.assertEqual(page_result["parse_version_id"], publication_id)
            self.assertEqual(page_result["snapshot_parse_version_id"], publication_id)
            self.assertEqual(page_result["source_version_id"], values[1])
            self.assertEqual(page_result["source_hash"], manifest["source_hash"])
            self.assertEqual(page_result["parent_parse_version_id"], page_result["manifest"]["parent_parse_version_id"])
            expected_page_parent = None if index == 0 else published[index - 1]
            self.assertEqual(page_result["parent_parse_version_id"], expected_page_parent)
            page_value = page_result["page"]
            self.assertEqual(page_value["source_page_index"], index)
            page_region_ids = {region["region_id"] for region in page_result["regions"]}
            page_object_ids = {obj["object_id"] for obj in page_result["objects"]}
            self.assertTrue(set(page_value["region_ids"]) <= page_region_ids)
            self.assertTrue(set(page_value["reading_order"]) <= page_object_ids)
            self.assertEqual(
                set(material_selector_parse_versions(page_result)),
                {publication_id},
            )
            page_resource_ids = {resource["resource_id"] for resource in page_result["resources"]}
            referenced_resource_ids = {page_value["preview_resource_id"]}
            referenced_resource_ids.update(
                obj.get("content", {}).get("resource_id")
                for obj in page_result["objects"]
                if obj.get("content", {}).get("resource_id") is not None
            )
            self.assertTrue(referenced_resource_ids <= page_resource_ids)
        for index, parse_version_id in enumerate(published):
            snapshot_path = self.root / "publication-chain" / "snapshots" / (parse_version_id + ".json")
            self.assertTrue(snapshot_path.is_file())
            snapshot = json.loads(snapshot_path.read_text())
            self.assertEqual(snapshot["parse_version_id"], parse_version_id)
            self.assertEqual(
                set(material_selector_parse_versions(snapshot)),
                {parse_version_id},
            )
            parent = snapshot.get("parent_parse_version_id")
            if parent is not None:
                self.assertTrue((snapshot_path.parent / (parent + ".json")).is_file())
            if index < 2:
                self.assertEqual(snapshot["provenance"]["publication_kind"], "page_checkpoint")
        first_snapshot = (self.root / "publication-chain" / "snapshots" / (published[0] + ".json")).read_bytes()
        replay = self.engine.convert(payload, task_id=values[3], resume=True)
        self.assertEqual(replay["manifest"]["parse_version_id"], values[2])
        self.assertEqual((self.root / "publication-chain" / "snapshots" / (published[0] + ".json")).read_bytes(), first_snapshot)

    def test_unknown_parent_snapshot_is_rejected_before_publication(self):
        source = self.root / "missing-parent.pdf"
        make_pdf(source, ["page"])
        values, payload = self.payload(source, stage_name="missing-parent")
        payload["parent_parse_version_id"] = str(uuid.uuid4())
        with self.assertRaises(EngineError) as error:
            self.engine.convert(payload, task_id=values[3])
        self.assertEqual(error.exception.code, "SOURCE_UNAVAILABLE")
        self.assertFalse((self.root / "missing-parent" / "manifest.json").exists())

    def test_duplicate_requested_page_is_rejected_before_publication(self):
        source = self.root / "duplicate-page.pdf"
        make_pdf(source, ["page"])
        values, payload = self.payload(source, pages=[0, 0], stage_name="duplicate-page")
        with self.assertRaises(EngineError) as error:
            self.engine.convert(payload, task_id=values[3])
        self.assertEqual(error.exception.code, "INVALID_ARGUMENT")
        self.assertFalse((self.root / "duplicate-page" / "manifest.json").exists())

    def test_manifest_rejects_source_anchor_outside_published_snapshot(self):
        source = self.root / "anchor-closure.pdf"
        make_pdf(source, ["page"])
        values, payload = self.payload(source, stage_name="anchor-closure")
        result = self.engine.convert(payload, task_id=values[3])
        manifest = json.loads(Path(result["manifest_path"]).read_text())
        manifest["objects"][0]["source_refs"][0]["selector"]["parse_version_id"] = str(uuid.uuid4())
        with self.assertRaises(EngineError) as error:
            validate_manifest(manifest, self.root / "anchor-closure")
        self.assertEqual(error.exception.code, "INVALID_RESULT")

    def test_manifest_rejects_search_unit_without_material_anchor(self):
        source = self.root / "search-anchor-closure.pdf"
        make_pdf(source, ["page"])
        values, payload = self.payload(source, stage_name="search-anchor-closure")
        result = self.engine.convert(payload, task_id=values[3])
        manifest = json.loads(Path(result["manifest_path"]).read_text())
        manifest["search_units"][0]["alternate_anchors"] = []
        with self.assertRaises(EngineError) as error:
            validate_manifest(manifest, self.root / "search-anchor-closure")
        self.assertEqual(error.exception.code, "INVALID_RESULT")

    def test_resume_skips_completed_page_and_retries_remaining_page(self):
        source = self.root / "two-pages.pdf"
        make_pdf(source, ["one", "two"])
        values, payload = self.payload(source, pages=[0, 1], stage_name="resume")
        cancelled = {"value": False}

        def stop_after_first(page):
            if page["source_page_index"] == 0 and not cancelled["value"]:
                cancelled["value"] = True
                self.engine.cancel(values[3])

        with self.assertRaises(Exception):
            self.engine.convert(payload, task_id=values[3], on_page=stop_after_first)
        checkpoint = json.loads((self.root / "resume" / "checkpoint.json").read_text())
        self.assertEqual(checkpoint["completed_pages"], [0])
        result = self.engine.convert(payload, task_id=values[3], resume=True)
        self.assertEqual([page["source_page_index"] for page in result["manifest"]["pages"]], [0, 1])
        self.assertEqual({unit["display_text"] for unit in result["manifest"]["search_units"]}, {"one", "two"})

    def test_pptx_text_has_native_object_and_protocol_rejects_old_major(self):
        source = self.root / "sample.pptx"
        make_pptx(source)
        values, payload = self.payload(source, profile="deep", stage_name="pptx")
        result = self.engine.convert(payload, task_id=values[3])
        self.assertEqual(result["manifest"]["objects"][0]["type"], "text")
        self.assertEqual(result["manifest"]["search_coverage"]["state"], "complete")
        protocol = EngineProtocol(self.engine)
        response = protocol.handle({"contract_version": "1.0.0", "request_id": str(uuid.uuid4()), "name": "handshake", "task_id": None, "payload": {}})[0]
        self.assertEqual(response["kind"], "error")
        self.assertEqual(response["payload"]["error"]["code"], "UNSUPPORTED_VERSION")

    def test_pptx_reconstructs_chart_values_and_connector_diagram(self):
        source = self.root / "rich.pptx"
        make_rich_pptx(source)
        values, payload = self.payload(source, profile="deep", stage_name="rich")
        manifest = self.engine.convert(payload, task_id=values[3])["manifest"]
        chart = next(obj for obj in manifest["objects"] if obj["type"] == "data_chart")
        diagram = next(obj for obj in manifest["objects"] if obj["type"] == "structure_diagram")
        self.assertEqual(manifest["completion"], "complete")
        self.assertEqual(chart["status"], "complete")
        self.assertEqual([point["y"] for point in chart["content"]["series"][0]["points"]], [1.0, 2.0])
        self.assertEqual(diagram["status"], "complete")
        self.assertEqual(diagram["content"]["edges"][0]["status"], "known")
        search_fields = {unit["field_path"] for unit in manifest["search_units"]}
        self.assertIn("content.nodes[{}].label".format(diagram["content"]["nodes"][0]["node_id"]), search_fields)
        chart_name_unit = next(unit for unit in manifest["search_units"] if unit["field_path"] == "content.series[0].name")
        self.assertEqual(chart_name_unit["anchor"]["selector"]["element_ids"], [chart["content"]["series"][0]["series_id"]])

    def test_resume_rejects_changed_source_and_parse_version(self):
        source = self.root / "resume-guard.pdf"
        make_pdf(source, ["stable"])
        values, payload = self.payload(source, stage_name="guard")
        self.engine.convert(payload, task_id=values[3])
        source.write_bytes(source.read_bytes() + b"changed")
        with self.assertRaises(EngineError) as changed:
            self.engine.convert(payload, task_id=values[3], resume=True)
        self.assertIn(changed.exception.code, ("IDEMPOTENCY_MISMATCH", "STALE_INPUT"))
        make_pdf(source, ["stable"])
        changed_parse = dict(payload)
        changed_parse["parse_version_id"] = str(uuid.uuid4())
        with self.assertRaises(EngineError) as parse_changed:
            self.engine.convert(changed_parse, task_id=values[3], resume=True)
        self.assertEqual(parse_changed.exception.code, "IDEMPOTENCY_MISMATCH")
        changed_parent = dict(payload)
        changed_parent["parent_parse_version_id"] = str(uuid.uuid4())
        with self.assertRaises(EngineError) as parent_changed:
            self.engine.convert(changed_parent, task_id=values[3], resume=True)
        self.assertEqual(parent_changed.exception.code, "IDEMPOTENCY_MISMATCH")

    def test_same_basename_uses_source_identity_and_image_is_partial(self):
        first = self.root / "one" / "same.pdf"
        second = self.root / "two" / "same.pdf"
        first.parent.mkdir()
        second.parent.mkdir()
        make_pdf(first, ["first file"])
        make_pdf(second, ["second file"])
        values1, payload1 = self.payload(first, stage_name="same-one")
        values2, payload2 = self.payload(second, stage_name="same-two")
        result1 = self.engine.convert(payload1, task_id=values1[3])["manifest"]
        result2 = self.engine.convert(payload2, task_id=values2[3])["manifest"]
        self.assertNotEqual(result1["source_hash"], result2["source_hash"])
        self.assertNotEqual(result1["objects"][0]["object_id"], result2["objects"][0]["object_id"])
        self.assertEqual(result1["search_units"][0]["display_text"], "first file")
        self.assertEqual(result2["search_units"][0]["display_text"], "second file")

        image = self.root / "scan.png"
        image.write_bytes(b"\x89PNG\r\n\x1a\n" + b"\x00\x00\x00\rIHDR" + (100).to_bytes(4, "big") + (80).to_bytes(4, "big") + b"\x08\x02\x00\x00\x00")
        values3, payload3 = self.payload(image, profile="deep", stage_name="image")
        image_manifest = self.engine.convert(payload3, task_id=values3[3])["manifest"]
        self.assertEqual(image_manifest["completion"], "partial")
        self.assertEqual(image_manifest["search_coverage"]["state"], "unavailable")
        self.assertEqual(image_manifest["objects"][0]["type"], "image")
        self.assertIn(image_manifest["pages"][0]["preview_resource_id"], {resource["resource_id"] for resource in image_manifest["resources"]})
        self.assertEqual(image_manifest["search_units"], [])

    def test_resource_path_escape_is_rejected(self):
        with self.assertRaises(EngineError):
            safe_relative_path("../outside.bin")
        with self.assertRaises(EngineError):
            safe_relative_path("resources/../../outside.bin")
        for path in ("resources/..", "a/b/..", "a/../b", "resources\\outside.bin"):
            with self.assertRaises(EngineError):
                safe_relative_path(path)

    def test_existing_unregistered_resource_is_never_overwritten(self):
        staging = self.root / "resource-store"
        ids = IdentityAllocator(str(uuid.uuid4()), str(uuid.uuid4()), {})
        store = ResourceStore(staging, ids)
        resource_id = ids.id("resource", "orphan")
        destination = staging / "resources" / (resource_id + ".bin")
        destination.write_bytes(b"old")
        with self.assertRaises(EngineError) as error:
            store.add_bytes("orphan", b"new", "application/octet-stream", "preview", [])
        self.assertEqual(error.exception.code, "CORRUPT_ASSET")
        self.assertEqual(destination.read_bytes(), b"old")

    def test_invalid_request_id_is_rejected_without_silent_replacement(self):
        protocol = EngineProtocol(self.engine)
        response = protocol.handle(
            {
                "contract_version": "2.0.0",
                "request_id": "req-123",
                "name": "handshake",
                "task_id": None,
                "payload": {},
            }
        )[0]
        self.assertEqual(response["kind"], "error")
        self.assertEqual(response["payload"]["error"]["code"], "INVALID_ARGUMENT")
        self.assertEqual(response["payload"]["error"]["details"]["request_id"], "req-123")

    def test_staging_uri_and_source_overlap_are_rejected(self):
        source = self.root / "overlap" / "source.pdf"
        source.parent.mkdir()
        make_pdf(source, ["page"])
        values, payload = self.payload(source, stage_name="overlap")
        with self.assertRaises(EngineError) as overlap:
            self.engine.convert(payload, task_id=values[3])
        self.assertEqual(overlap.exception.code, "INVALID_ARGUMENT")
        values_uri, payload_uri = self.payload(self.root / "uri-source.pdf", stage_name="uri-stage")
        make_pdf(Path(payload_uri["source_handle"]), ["page"])
        payload_uri["staging_handle"] = "file://" + payload_uri["staging_handle"]
        with self.assertRaises(EngineError) as uri:
            self.engine.convert(payload_uri, task_id=values_uri[3])
        self.assertEqual(uri.exception.code, "INVALID_ARGUMENT")

    def test_resume_draft_handles_nullable_anchor_fallback(self):
        source = self.root / "nullable-fallback.pdf"
        make_pdf(source, ["page"])
        values, payload = self.payload(source, stage_name="nullable-fallback")
        manifest = self.engine.convert(payload, task_id=values[3])["manifest"]
        for unit in manifest["search_units"]:
            unit["anchor"]["fallback"] = None
        draft = _draft_from_manifest(manifest, 0)
        self.assertIsNotNone(draft)
        self.assertEqual(len(draft.search_units), 1)

    def test_jsonl_subprocess_flushes_progress_cancel_and_resume(self):
        source = self.root / "process.pdf"
        make_pdf(source, ["page-{}".format(index) for index in range(20)])
        values, payload = self.payload(source, pages=list(range(20)), stage_name="process")
        environment = os.environ.copy()
        environment["PYTHONPATH"] = str(Path(__file__).parents[1])
        process = subprocess.Popen(
            [sys.executable, "-m", "material_engine"],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            bufsize=1,
            env=environment,
            cwd=str(Path.cwd()),
        )
        line_queue = queue.Queue()

        def attach_reader():
            current_queue = line_queue
            current_stream = process.stdout

            def read_lines():
                for line in current_stream:
                    current_queue.put(line)
                current_queue.put(None)

            thread = threading.Thread(target=read_lines, daemon=True)
            thread.start()

        def send(value):
            process.stdin.write(json.dumps(value, ensure_ascii=False) + "\n")
            process.stdin.flush()

        def read_until(predicate, timeout=30):
            found = []
            deadline = time.monotonic() + timeout
            while time.monotonic() < deadline:
                try:
                    line = line_queue.get(timeout=max(0.05, deadline - time.monotonic()))
                except queue.Empty:
                    continue
                if line is None:
                    break
                value = json.loads(line)
                found.append(value)
                if predicate(value):
                    return found
            self.fail("sidecar did not emit expected event; received {}".format([item.get("kind") for item in found]))

        try:
            attach_reader()
            send({"contract_version": "2.0.0", "request_id": str(uuid.uuid4()), "name": "handshake", "task_id": None, "payload": {}})
            ready = read_until(lambda value: value.get("kind") == "ready")
            self.assertEqual(ready[-1]["payload"]["material_format"], "focaus.material/2.0.0")
            send({"contract_version": "2.0.0", "request_id": str(uuid.uuid4()), "name": "convert", "task_id": values[3], "payload": payload})
            progress = read_until(lambda value: value.get("kind") == "checkpoint" and value.get("payload", {}).get("state") == "running")
            running = progress[-1]
            self.assertIn(0, running["payload"]["completed_pages"])
            send({"contract_version": "2.0.0", "request_id": str(uuid.uuid4()), "name": "cancel", "task_id": values[3], "payload": {}})
            cancelled = read_until(lambda value: value.get("kind") == "error" and value.get("payload", {}).get("error", {}).get("code") == "CANCELLED")
            self.assertTrue(any(value.get("kind") == "page_result" for value in progress + cancelled))
            checkpoint = json.loads((self.root / "process" / "checkpoint.json").read_text())
            self.assertLess(len(checkpoint["completed_pages"]), 20)
            # Close the first process and recover from its on-disk checkpoint
            # in a fresh process, which is the host restart path.
            process.stdin.close()
            process.wait(timeout=30)
            process.stdout.close()
            process.stderr.close()
            process = subprocess.Popen(
                [sys.executable, "-m", "material_engine"],
                stdin=subprocess.PIPE,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                text=True,
                bufsize=1,
                env=environment,
                cwd=str(Path.cwd()),
            )
            line_queue = queue.Queue()
            attach_reader()
            send({"contract_version": "2.0.0", "request_id": str(uuid.uuid4()), "name": "handshake", "task_id": None, "payload": {}})
            read_until(lambda value: value.get("kind") == "ready")
            resume_request = dict(payload)
            send({"contract_version": "2.0.0", "request_id": str(uuid.uuid4()), "name": "resume", "task_id": values[3], "payload": resume_request})
            completed = read_until(lambda value: value.get("kind") == "completed", timeout=45)
            manifest = completed[-1]["payload"]["manifest"]
            self.assertEqual(len(manifest["pages"]), 20)
            self.assertEqual(manifest["completion"], "complete")
        finally:
            if process.stdin:
                process.stdin.close()
            process.wait(timeout=30)
            if process.stdout:
                process.stdout.close()
            if process.stderr:
                process.stderr.close()

    def test_cancel_before_worker_registration_is_preserved(self):
        source = self.root / "early-cancel.pdf"
        make_pdf(source, ["early"])
        values, payload = self.payload(source, stage_name="early-cancel")
        protocol = EngineProtocol()
        cancel = {"contract_version": "2.0.0", "request_id": str(uuid.uuid4()), "name": "cancel", "task_id": values[3], "payload": {}}
        self.assertEqual(protocol.handle(cancel)[0]["kind"], "checkpoint")
        convert = {"contract_version": "2.0.0", "request_id": str(uuid.uuid4()), "name": "convert", "task_id": values[3], "payload": payload}
        responses = protocol.handle(convert)
        self.assertEqual(responses[-1]["kind"], "error")
        self.assertEqual(responses[-1]["payload"]["error"]["code"], "CANCELLED")

    def test_jsonl_progress_identity_order_cancel_resume_and_final_parent(self):
        source = self.root / "progress-shape.pdf"
        make_pdf(source, ["page-{}".format(index) for index in range(20)])
        values, payload = self.payload(source, pages=list(range(20)), stage_name="progress-shape")
        environment = os.environ.copy()
        environment["PYTHONPATH"] = str(Path(__file__).parents[1])
        process = None
        line_queue = None

        def start_process():
            nonlocal process, line_queue
            process = subprocess.Popen(
                [sys.executable, "-m", "material_engine"],
                stdin=subprocess.PIPE,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                text=True,
                bufsize=1,
                env=environment,
                cwd=str(Path.cwd()),
            )
            line_queue = queue.Queue()
            current_stream = process.stdout

            def read_lines():
                for line in current_stream:
                    line_queue.put(line)
                line_queue.put(None)

            threading.Thread(target=read_lines, daemon=True).start()

        def send(request):
            process.stdin.write(json.dumps(request, ensure_ascii=False) + "\n")
            process.stdin.flush()

        def read_until(predicate, timeout=30):
            found = []
            deadline = time.monotonic() + timeout
            while time.monotonic() < deadline:
                try:
                    line = line_queue.get(timeout=max(0.05, deadline - time.monotonic()))
                except queue.Empty:
                    continue
                if line is None:
                    break
                value = json.loads(line)
                found.append(value)
                if predicate(value):
                    return found
            self.fail("sidecar did not emit expected progress event; received {}".format([item.get("kind") for item in found]))

        def stop_process():
            if process is None:
                return
            if process.stdin:
                process.stdin.close()
            process.wait(timeout=30)
            if process.stdout:
                process.stdout.close()
            if process.stderr:
                process.stderr.close()

        try:
            start_process()
            handshake_request = {"contract_version": "2.0.0", "request_id": str(uuid.uuid4()), "name": "handshake", "task_id": None, "payload": {}}
            send(handshake_request)
            ready_events = read_until(lambda value: value.get("kind") == "ready")
            ready = ready_events[-1]
            self.assertEqual(ready["request_id"], handshake_request["request_id"])
            self.assertIsNone(ready["task_id"])

            convert_request = {"contract_version": "2.0.0", "request_id": str(uuid.uuid4()), "name": "convert", "task_id": values[3], "payload": payload}
            send(convert_request)
            first_page_events = read_until(
                lambda value: value.get("kind") == "checkpoint" and value.get("payload", {}).get("state") == "running"
            )
            page_result = next(value for value in first_page_events if value.get("kind") == "page_result")
            running_checkpoint = next(value for value in first_page_events if value.get("kind") == "checkpoint")
            self.assertLess(first_page_events.index(page_result), first_page_events.index(running_checkpoint))
            self.assertEqual(page_result["request_id"], convert_request["request_id"])
            self.assertEqual(page_result["task_id"], values[3])
            page_payload = page_result["payload"]
            self.assertEqual(page_payload["material_id"], values[0])
            self.assertEqual(page_payload["source_version_id"], values[1])
            self.assertEqual(page_payload["source_hash"], page_payload["manifest"]["source_hash"])
            self.assertEqual(page_payload["parse_version_id"], page_payload["manifest"]["parse_version_id"])
            self.assertEqual(page_payload["parent_parse_version_id"], page_payload["manifest"]["parent_parse_version_id"])
            self.assertEqual(page_payload["snapshot_parse_version_id"], page_payload["parse_version_id"])
            self.assertEqual(page_payload["manifest"]["provenance"]["publication_kind"], "page_checkpoint")
            self.assertEqual(page_payload["progress"]["material_id"], values[0])
            self.assertEqual(page_payload["progress"]["state"], "running")
            self.assertEqual(page_payload["progress"]["publication_parse_version_id"], page_payload["parse_version_id"])
            self.assertEqual(page_payload["progress"]["publication_parent_parse_version_id"], page_payload["parent_parse_version_id"])

            cancel_request = {"contract_version": "2.0.0", "request_id": str(uuid.uuid4()), "name": "cancel", "task_id": values[3], "payload": {}}
            send(cancel_request)
            cancelled_events = read_until(
                lambda value: value.get("kind") == "error" and value.get("payload", {}).get("error", {}).get("code") == "CANCELLED"
            )
            cancel_checkpoint = next(
                value
                for value in cancelled_events
                if value.get("kind") == "checkpoint" and value.get("payload", {}).get("state") == "cancel_requested"
            )
            self.assertEqual(cancel_checkpoint["request_id"], cancel_request["request_id"])
            self.assertEqual(cancel_checkpoint["task_id"], values[3])
            checkpoint = json.loads((self.root / "progress-shape" / "checkpoint.json").read_text())
            self.assertEqual(checkpoint["material_id"], values[0])
            self.assertEqual(checkpoint["source_version_id"], values[1])
            self.assertLess(len(checkpoint["completed_pages"]), 20)
            stop_process()
            process = None

            start_process()
            resume_handshake = {"contract_version": "2.0.0", "request_id": str(uuid.uuid4()), "name": "handshake", "task_id": None, "payload": {}}
            send(resume_handshake)
            ready = read_until(lambda value: value.get("kind") == "ready")[-1]
            self.assertEqual(ready["request_id"], resume_handshake["request_id"])
            resume_request = {"contract_version": "2.0.0", "request_id": str(uuid.uuid4()), "name": "resume", "task_id": values[3], "payload": dict(payload)}
            send(resume_request)
            resume_events = read_until(lambda value: value.get("kind") == "completed", timeout=45)
            for event in resume_events:
                self.assertEqual(event["request_id"], resume_request["request_id"])
                self.assertEqual(event["task_id"], values[3])
            page_positions = [index for index, event in enumerate(resume_events) if event.get("kind") == "page_result"]
            running_positions = [
                index
                for index, event in enumerate(resume_events)
                if event.get("kind") == "checkpoint" and event.get("payload", {}).get("state") == "running"
            ]
            completed_checkpoint_position = next(
                index
                for index, event in enumerate(resume_events)
                if event.get("kind") == "checkpoint" and event.get("payload", {}).get("state") == "completed"
            )
            completed_position = next(index for index, event in enumerate(resume_events) if event.get("kind") == "completed")
            self.assertTrue(page_positions)
            self.assertTrue(running_positions)
            self.assertLess(page_positions[0], running_positions[0])
            self.assertLess(max(page_positions), completed_checkpoint_position)
            self.assertLess(completed_checkpoint_position, completed_position)
            completed_payload = resume_events[completed_position]["payload"]
            final_manifest = completed_payload["manifest"]
            self.assertEqual(completed_payload["material_id"], values[0])
            self.assertEqual(completed_payload["source_version_id"], values[1])
            self.assertEqual(completed_payload["source_hash"], final_manifest["source_hash"])
            self.assertEqual(completed_payload["parse_version_id"], final_manifest["parse_version_id"])
            self.assertIsNone(completed_payload["parent_parse_version_id"])
            self.assertEqual(completed_payload["manifest"]["provenance"]["publication_kind"], "final")
            self.assertEqual(completed_payload["progress"]["state"], "completed")
            self.assertEqual(completed_payload["progress"]["parse_version_id"], final_manifest["parse_version_id"])
            self.assertEqual(completed_payload["progress"]["publication_parse_version_id"], completed_payload["parse_version_id"])
            self.assertEqual(completed_payload["progress"]["publication_parent_parse_version_id"], completed_payload["parent_parse_version_id"])
            self.assertEqual(len(final_manifest["pages"]), 20)
        finally:
            if process is not None:
                stop_process()

    def test_jsonl_escapes_backslash_nul_and_c1_text(self):
        source = self.root / "control-text.pdf"
        make_pdf(source, ["literal\\u0000\\\\tail\x00\x81学习"])
        values, payload = self.payload(source, stage_name="control-text")
        environment = os.environ.copy()
        environment["PYTHONPATH"] = str(Path(__file__).parents[1])
        request = {
            "contract_version": "2.0.0",
            "request_id": str(uuid.uuid4()),
            "name": "convert",
            "task_id": values[3],
            "payload": payload,
        }
        process = subprocess.Popen(
            [sys.executable, "-m", "material_engine"],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            env=environment,
            cwd=str(Path.cwd()),
        )
        try:
            stdout, stderr = process.communicate(json.dumps(request, ensure_ascii=True) + "\n", timeout=30)
            self.assertEqual(process.returncode, 0, stderr)
            responses = [json.loads(line) for line in stdout.splitlines() if line.strip()]
            self.assertTrue(any(response["kind"] == "page_result" for response in responses))
            completed = next(response for response in responses if response["kind"] == "completed")
            self.assertEqual(completed["payload"]["manifest"]["completion"], "complete")
        finally:
            if process.poll() is None:
                process.kill()
                process.wait(timeout=10)
            if process.stdout:
                process.stdout.close()
            if process.stderr:
                process.stderr.close()
