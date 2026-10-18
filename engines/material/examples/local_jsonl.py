#!/usr/bin/env python3
"""Emit one valid local EngineRequest JSON line for the sidecar.

Examples:
  python3 examples/local_jsonl.py --mode handshake
  python3 examples/local_jsonl.py --mode convert --source /tmp/lecture.pdf --staging /tmp/focaus-task
  python3 examples/local_jsonl.py --mode resume --source /tmp/lecture.pdf --staging /tmp/focaus-task --task-id ... --material-id ... --source-version-id ... --parse-version-id ...

The IDs are printed in the request when omitted.  Save them for a later
resume; the sidecar also verifies the source hash and checkpoint identity.
"""

import argparse
import json
import sys
import uuid


def new_id() -> str:
    return str(uuid.uuid4())


def main() -> int:
    parser = argparse.ArgumentParser(description="Emit a Focaus material EngineRequest JSON line")
    parser.add_argument("--mode", choices=("handshake", "convert", "resume"), required=True)
    parser.add_argument("--source")
    parser.add_argument("--staging")
    parser.add_argument("--material-id", default=None)
    parser.add_argument("--source-version-id", default=None)
    parser.add_argument("--parse-version-id", default=None)
    parser.add_argument("--task-id", default=None)
    parser.add_argument("--profile", choices=("light", "deep"), default="light")
    parser.add_argument("--pages", nargs="*", type=int, default=[])
    parser.add_argument("--budget-ms", type=int, default=600000)
    args = parser.parse_args()
    request_id = new_id()
    if args.mode == "handshake":
        value = {"contract_version": "2.0.0", "request_id": request_id, "name": "handshake", "task_id": None, "payload": {}}
    else:
        missing = [name for name, value in (("--source", args.source), ("--staging", args.staging)) if not value]
        if missing:
            parser.error("{} required for {}".format(", ".join(missing), args.mode))
        payload = {
            "material_id": args.material_id or new_id(),
            "source_version_id": args.source_version_id or new_id(),
            "parse_version_id": args.parse_version_id or new_id(),
            "source_handle": args.source,
            "staging_handle": args.staging,
            "identity_pool": {},
            "profile": args.profile,
            "pages": args.pages,
            "corrections": [],
            "budget_ms": args.budget_ms,
        }
        value = {"contract_version": "2.0.0", "request_id": request_id, "name": args.mode, "task_id": args.task_id or new_id(), "payload": payload}
    json.dump(value, sys.stdout, ensure_ascii=False, separators=(",", ":"))
    sys.stdout.write("\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
