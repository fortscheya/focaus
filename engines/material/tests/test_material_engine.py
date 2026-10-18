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

from material_engine.common import content_hash, safe_relative_path
from material_engine.engine import MaterialEngine
from material_engine.errors import EngineError
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
