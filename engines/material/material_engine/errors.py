"""Stable error values exposed by the material sidecar."""

from typing import Dict, Optional


class EngineError(ValueError):
    """An error that can be represented by the shared SafeError contract."""

    def __init__(
        self,
        code: str,
        message: str,
        *,
        retryable: bool = False,
        scope: str = "task",
        details: Optional[Dict[str, str]] = None,
    ) -> None:
        super().__init__(message)
        self.code = code
        self.message = message
        self.retryable = retryable
        self.scope = scope
        self.details = {str(k): str(v) for k, v in (details or {}).items()}

    def as_safe_error(self) -> dict:
        value = {
            "code": self.code,
            "retryable": self.retryable,
            "scope": self.scope,
            "message": self.message,
        }
        if self.details:
            value["details"] = self.details
        return value


def safe_error(
    code: str,
    message: str,
    *,
    retryable: bool = False,
    scope: str = "task",
    details: Optional[Dict[str, str]] = None,
) -> dict:
    return EngineError(
        code,
        message,
        retryable=retryable,
        scope=scope,
        details=details,
    ).as_safe_error()
