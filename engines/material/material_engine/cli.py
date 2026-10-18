"""Executable JSONL entrypoint for the material sidecar."""

from __future__ import annotations

import json
import sys

from .errors import EngineError
from .protocol import EngineProtocol


def main() -> int:
    protocol = EngineProtocol()
    for raw_line in sys.stdin:
        line = raw_line.strip()
        if not line:
            continue
        try:
            request = json.loads(line)
        except json.JSONDecodeError:
            response = protocol._invalid_response("00000000-0000-0000-0000-000000000000", EngineError("INVALID_ARGUMENT", "input line is not valid JSON", scope="command"))
            sys.stdout.write(json.dumps(response, ensure_ascii=False, separators=(",", ":")) + "\n")
            sys.stdout.flush()
            continue
        for response in protocol.handle(request):
            sys.stdout.write(json.dumps(response, ensure_ascii=False, separators=(",", ":")) + "\n")
            sys.stdout.flush()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
