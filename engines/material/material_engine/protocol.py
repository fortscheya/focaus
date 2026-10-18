"""JSONL EngineRequest/EngineResponse protocol adapter."""

from __future__ import annotations

import uuid
from typing import Any, Dict, List, Mapping, Optional

from .common import CONTRACT_VERSION, is_uuid, require_uuid
from .engine import ENGINE_BUILD, MaterialEngine
from .errors import EngineError, safe_error


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

    def _invalid_response(self, request_id: str, error: EngineError, task_id: Optional[str] = None) -> dict:
        return _response(request_id, task_id, "error", {"error": error.as_safe_error(), "engine_build": ENGINE_BUILD})

    def handle(self, request: Mapping[str, Any]) -> List[dict]:
        request_id = str(request.get("request_id")) if isinstance(request, Mapping) else str(uuid.uuid4())
        if not is_uuid(request_id):
            request_id = str(uuid.uuid4())
        task_raw = request.get("task_id") if isinstance(request, Mapping) else None
        task_id = str(task_raw) if task_raw is not None and is_uuid(task_raw) else None
        events: List[dict] = []
        try:
            if not isinstance(request, Mapping):
                raise EngineError("INVALID_ARGUMENT", "EngineRequest must be an object", scope="command")
            if request.get("contract_version") != CONTRACT_VERSION:
                raise EngineError("UNSUPPORTED_VERSION", "engine accepts contract version 2.0.0 only", scope="command")
            name = request.get("name")
            payload = request.get("payload") or {}
            if name == "handshake":
                return [
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
                ]
            if name not in ("convert", "resume", "cancel"):
                raise EngineError("INVALID_ARGUMENT", "unknown engine request name", scope="command", details={"name": str(name)})
            task_id = require_uuid(request.get("task_id"), "task_id")
            if name == "cancel":
                self.engine.cancel(task_id)
                return [_response(request_id, task_id, "checkpoint", {"task_id": task_id, "state": "cancel_requested"})]
            def on_page(value: dict) -> None:
                events.append(_response(request_id, task_id, "page_result", value))

            def on_checkpoint(value: dict) -> None:
                events.append(_response(request_id, task_id, "checkpoint", value))

            result = self.engine.convert(payload, task_id=task_id, resume=name == "resume", on_page=on_page, on_checkpoint=on_checkpoint)
            events.append(_response(request_id, task_id, "completed", result))
            return events
        except EngineError as exc:
            events.append(_response(request_id, task_id, "error", {"error": exc.as_safe_error(), "engine_build": ENGINE_BUILD}))
            return events
        except Exception as exc:  # pragma: no cover - protocol boundary guard
            error = EngineError("PROVIDER_ERROR", "material engine failed unexpectedly", retryable=True, scope="task", details={"exception": type(exc).__name__})
            events.append(self._invalid_response(request_id, error, task_id))
            return events
