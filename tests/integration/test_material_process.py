"""Exercise E's actual JSONL process against C's public schema.

Inputs are synthetic and created under a temporary directory. No model, user
asset, database, or network service participates in this local boundary check.
"""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
import uuid

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "schemas"))
from validate import validate, search_text_hash


def identity():
    return str(uuid.uuid4())


def request(name, payload, task=None, version="2.0.0"):
    return {"contract_version": version, "request_id": identity(),
            "task_id": task, "name": name, "payload": payload}


class MaterialProcess(unittest.TestCase):
    def run_sidecar(self, messages):
        environment = os.environ.copy()
        environment["PYTHONPATH"] = str(ROOT / "engines/material")
        result = subprocess.run(
            [sys.executable, "-m", "material_engine"],
            input="".join(json.dumps(value, ensure_ascii=False) + "\n" for value in messages),
            cwd=ROOT, env=environment, text=True, capture_output=True,
            timeout=30, check=True,
        )
        self.assertEqual(result.stderr, "")
        responses = [json.loads(line) for line in result.stdout.splitlines()]
        for value in responses:
            validate("EngineResponse", value)
            self.assertIn(value["request_id"], {r["request_id"] for r in messages})
        return responses

    def test_process_handshake_convert_and_restart_resume(self):
        with tempfile.TemporaryDirectory(prefix="focaus-engine-wire-") as folder:
            root = Path(folder).resolve()
            source = root / "lecture.txt"
            # Unicode must survive the process wire without normalization or loss.
            original = "Straße 学习 e\u0301 😀\nConvolution stride=2, padding=1.\n"
            source.write_text(original, encoding="utf-8")
            task = identity()
            payload = {
                "material_id": identity(), "source_version_id": identity(),
                "parse_version_id": identity(), "source_handle": str(source),
                "staging_handle": str(root / "stage"), "identity_pool": {},
                "profile": "light", "pages": [], "corrections": [], "budget_ms": 5000,
            }
            handshake = request("handshake", {})
            conversion = request("convert", payload, task)
            for value in (handshake, conversion):
                validate("EngineRequest", value)
            responses = self.run_sidecar([handshake, conversion])
            self.assertEqual(responses[0]["kind"], "ready")
            self.assertFalse(responses[0]["payload"]["cloud_access"])
            self.assertEqual(responses[-1]["kind"], "completed")
            converted = [r for r in responses if r["request_id"] == conversion["request_id"]]
            self.assertTrue(all(r["task_id"] == task for r in converted))
            self.assertIn("checkpoint", {r["kind"] for r in converted})
            self.assertIn("page_result", {r["kind"] for r in converted})
            manifest = converted[-1]["payload"]["manifest"]
            validate("MaterialManifest", manifest)
            for unit in manifest["search_units"]:
                self.assertEqual(unit["content_hash"], search_text_hash(unit["display_text"]))
            self.assertEqual(manifest["source_hash"], hashlib.sha256(source.read_bytes()).hexdigest())
            self.assertEqual(manifest["parse_version_id"], payload["parse_version_id"])
            self.assertIn("Straße 学习 e\u0301 😀", "\n".join(u["display_text"] for u in manifest["search_units"]))
            for resource in manifest["resources"]:
                path = (root / "stage" / resource["relative_path"]).resolve()
                self.assertTrue(path.is_relative_to(root / "stage"))
                self.assertTrue(path.is_file())
                self.assertEqual(hashlib.sha256(path.read_bytes()).hexdigest(), resource["sha256"])
            # Resume runs in a new interpreter: no in-memory engine state survives.
            resumed = self.run_sidecar([request("resume", payload, task)])[-1]
            self.assertEqual(resumed["kind"], "completed")
            validate("MaterialManifest", resumed["payload"]["manifest"])
            self.assertEqual(manifest["search_units"], resumed["payload"]["manifest"]["search_units"])
            self.assertEqual(source.read_text(encoding="utf-8"), original)

    def test_unknown_contract_rejected_without_process_failure(self):
        bad = request("handshake", {}, version="99.0.0")
        good = request("handshake", {})
        responses = self.run_sidecar([bad, good])
        self.assertEqual(responses[0]["kind"], "error")
        validate("SafeError", responses[0]["payload"]["error"])
        self.assertEqual(responses[0]["payload"]["error"]["code"], "UNSUPPORTED_VERSION")
        self.assertEqual(responses[1]["kind"], "ready")


if __name__ == "__main__":
    unittest.main()
