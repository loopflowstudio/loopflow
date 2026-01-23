"""JSON-over-newline protocol for socket communication."""

import json
from dataclasses import dataclass
from typing import Any


@dataclass
class Request:
    method: str
    params: dict[str, Any]
    id: str | None = None

    @classmethod
    def parse(cls, line: str) -> "Request":
        data = json.loads(line)
        return cls(
            method=data["method"],
            params=data.get("params", {}),
            id=data.get("id"),
        )


@dataclass
class Response:
    ok: bool
    result: Any = None
    error: str | None = None
    id: str | None = None

    def serialize(self) -> str:
        data: dict[str, Any] = {"ok": self.ok}
        if self.ok:
            data["result"] = self.result
        else:
            data["error"] = self.error
        if self.id is not None:
            data["id"] = self.id
        return json.dumps(data)


@dataclass
class Event:
    event: str
    data: dict[str, Any]

    def serialize(self) -> str:
        return json.dumps({"event": self.event, "data": self.data})


def success(result: Any, id: str | None = None) -> Response:
    return Response(ok=True, result=result, id=id)


def error(message: str, id: str | None = None) -> Response:
    return Response(ok=False, error=message, id=id)
