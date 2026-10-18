"""Executable JSONL entrypoint for the material sidecar."""

from __future__ import annotations

import json
import queue
import sys
import threading
from typing import Any

from .common import CONTRACT_VERSION, is_uuid
from .errors import EngineError
from .protocol import EngineProtocol


def main() -> int:
    protocol = EngineProtocol()
    incoming = queue.Queue(maxsize=8)
    outgoing = queue.Queue(maxsize=32)

    def read_input() -> None:
        try:
            for raw_line in sys.stdin:
                incoming.put(("line", raw_line), block=True)
        finally:
            incoming.put(("eof", None), block=True)

    reader = threading.Thread(target=read_input, name="material-jsonl-reader", daemon=True)
    reader.start()
    worker = None
    active_task = None
    input_closed = False

    def emit(response: dict) -> None:
        outgoing.put(("response", response), block=True)

    def start_worker(request: dict) -> None:
        nonlocal worker, active_task
        active_task = request.get("task_id")

        def run() -> None:
            protocol.handle_stream(request, emit)
            outgoing.put(("done", request.get("task_id")), block=True)

        worker = threading.Thread(target=run, name="material-jsonl-worker", daemon=True)
        worker.start()

    def write(response: dict) -> None:
        # ASCII escaping keeps backslashes followed by ``u`` and all control
        # ranges unambiguous for hosts that parse each JSONL line directly.
        sys.stdout.write(json.dumps(response, ensure_ascii=True, separators=(",", ":")) + "\n")
        sys.stdout.flush()

    while True:
        # Drain worker responses before waiting for input.  This makes page and
        # checkpoint events visible while stdin is otherwise idle.
        for _ in range(8):
            try:
                kind, value = outgoing.get_nowait()
            except queue.Empty:
                break
            if kind == "response":
                write(value)
            elif kind == "done":
                active_task = None
                worker = None

        if input_closed and active_task is None:
            break
        try:
            kind, raw_value = incoming.get(timeout=0.05)
        except queue.Empty:
            continue
        if kind == "eof":
            input_closed = True
            continue
        line = raw_value.strip()
        if not line:
            continue
        try:
            request: Any = json.loads(line)
        except json.JSONDecodeError:
            write(protocol.error_response({}, EngineError("INVALID_ARGUMENT", "input line is not valid JSON", scope="command")))
            continue
        if not isinstance(request, dict):
            write(protocol.error_response(request, EngineError("INVALID_ARGUMENT", "EngineRequest must be an object", scope="command")))
            continue
        if request.get("contract_version") != CONTRACT_VERSION or not is_uuid(request.get("request_id")):
            for response in protocol.handle(request):
                write(response)
            continue
        name = request.get("name")
        if name in ("convert", "resume"):
            if not is_uuid(request.get("task_id")):
                write(protocol.error_response(request, EngineError("INVALID_ARGUMENT", "task_id must be a UUID", scope="command")))
            elif active_task is not None:
                write(protocol.error_response(request, EngineError("CONFLICT", "one conversion task is already running", retryable=True, scope="task")))
            else:
                start_worker(request)
        elif name == "cancel":
            # The reader loop handles cancel while the single worker is active.
            for response in protocol.handle(request):
                write(response)
        else:
            for response in protocol.handle(request):
                write(response)

    # If EOF arrived while a worker was active, drain its terminal responses.
    while active_task is not None or not outgoing.empty():
        try:
            kind, value = outgoing.get(timeout=0.05)
        except queue.Empty:
            continue
        if kind == "response":
            write(value)
        else:
            active_task = None
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
