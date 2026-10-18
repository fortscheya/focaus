"""JSONL EngineRequest/EngineResponse protocol adapter."""

from __future__ import annotations

import uuid
from typing import Any, Callable, List, Mapping, Optional

from .common import CONTRACT_VERSION, is_uuid, require_uuid
from .engine import ENGINE_BUILD, MaterialEngine
from .errors import EngineError


ResponseSink = Callable[[dict], None]


def _response(request_id: str, task_id: Optional[str], kind: str, payload: dict) -> dict:
    return {
        "contract_version": CONTRACT_VERSION,
        "request_id": request_id,
        "task_id": task_id,
        "kind": kind,
        "payload": payload,
    }


class EngineProtocol:
    def __init__(self, engine: Optional[MaterialEngine] = None) -> None:
        self.engine = engine or MaterialEngine()

    def _request_ids(self, request: Any) -> tuple:
        if isinstance(request, Mapping):
            request_id = str(request.get("request_id"))
            task_raw = request.get("task_id")
        else:
            request_id = str(uuid.uuid4())
            task_raw = None
        if not is_uuid(request_id):
            request_id = str(uuid.uuid4())
        task_id = str(task_raw) if task_raw is not None and is_uuid(task_raw) else None
        return request_id, task_id

    def error_response(self, request: Any, error: EngineError) -> dict:
        request_id, task_id = self._request_ids(request)
        return _response(request_id, task_id, "error", {"error": error.as_safe_error(), "engine_build": ENGINE_BUILD})

    def _dispatch(self, request: Mapping[str, Any], emit: ResponseSink) -> None:
        request_id, task_id = self._request_ids(request)
        if request.get("contract_version") != CONTRACT_VERSION:
            raise EngineError("UNSUPPORTED_VERSION", "engine accepts contract version 2.0.0 only", scope="command")
        name = request.get("name")
        payload = request.get("payload") or {}
        if name == "handshake":
            emit(
                _response(
                    request_id,
                    None,
                    "ready",
                    {
                        "engine_build": ENGINE_BUILD,
                        "contract_version": CONTRACT_VERSION,
                        "material_format": "focaus.material/2.0.0",
                        "capabilities": self.engine.capabilities(),
                        "write_boundary": "staging_handle_only",
                        "cloud_access": False,
                    },
                )
            )
            return
        if name not in ("convert", "resume", "cancel"):
            raise EngineError("INVALID_ARGUMENT", "unknown engine request name", scope="command", details={"name": str(name)})
        task_id = require_uuid(request.get("task_id"), "task_id")
        if name == "cancel":
            self.engine.cancel(task_id)
            emit(_response(request_id, task_id, "checkpoint", {"task_id": task_id, "state": "cancel_requested"}))
            return

        def on_page(value: dict) -> None:
            emit(_response(request_id, task_id, "page_result", value))

        def on_checkpoint(value: dict) -> None:
            emit(_response(request_id, task_id, "checkpoint", value))

        result = self.engine.convert(
            payload,
            task_id=task_id,
            resume=name == "resume",
            on_page=on_page,
            on_checkpoint=on_checkpoint,
        )
        emit(_response(request_id, task_id, "completed", result))

    def handle_stream(self, request: Any, emit: ResponseSink) -> None:
        """Process one request and emit each response as soon as it is ready."""
        try:
            if not isinstance(request, Mapping):
                raise EngineError("INVALID_ARGUMENT", "EngineRequest must be an object", scope="command")
            self._dispatch(request, emit)
        except EngineError as exc:
            emit(self.error_response(request, exc))
        except Exception as exc:  # pragma: no cover - protocol boundary guard
            error = EngineError(
                "PROVIDER_ERROR",
                "material engine failed unexpectedly",
                retryable=True,
                scope="task",
                details={"exception": type(exc).__name__},
            )
            emit(self.error_response(request, error))

    def handle(self, request: Any) -> List[dict]:
        """Collect the stream for synchronous callers and unit tests."""
        responses: List[dict] = []
        self.handle_stream(request, responses.append)
        return responses
