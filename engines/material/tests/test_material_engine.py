import json
import shutil
import tempfile
import uuid
import zipfile
from pathlib import Path
from unittest import TestCase

from material_engine.common import content_hash
from material_engine.engine import MaterialEngine
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
        for resource in manifest["resources"]:
            self.assertTrue((self.root / "stage" / resource["relative_path"]).is_file())

    def test_correction_replay_updates_search_text_without_mutating_source(self):
        source = self.root / "sample.pdf"
        make_pdf(source, ["before"])
        values, payload = self.payload(source, stage_name="correction")
        first = self.engine.convert(payload, task_id=values[3])
        object_value = first["manifest"]["objects"][0]
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
        payload2["corrections"] = [correction]
        result = self.engine.convert(payload2, task_id=values2[3])
        self.assertEqual(result["manifest"]["objects"][0]["content"]["runs"][0]["text"], "after")
        self.assertEqual(result["manifest"]["search_units"][0]["display_text"], "after")
        self.assertEqual(result["manifest"]["corrections_applied"][0]["replay_status"], "applied")
        self.assertEqual(source.read_bytes().count(b"before"), 1)

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
