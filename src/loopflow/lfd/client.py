"""Client for connecting to lfd daemon."""

import asyncio
import json
import socket
from pathlib import Path
from typing import Any, AsyncIterator

from loopflow.lfd.models import Session, SessionStatus

SOCKET_PATH = Path.home() / ".lf" / "lfd.sock"


class DaemonClient:
    """Client for connecting to lfd daemon from CLI or tests."""

    def __init__(self, socket_path: Path | None = None):
        self.socket_path = socket_path or SOCKET_PATH
        self._reader: asyncio.StreamReader | None = None
        self._writer: asyncio.StreamWriter | None = None

    async def connect(self) -> None:
        """Connect to the daemon socket."""
        self._reader, self._writer = await asyncio.open_unix_connection(
            str(self.socket_path)
        )

    async def close(self) -> None:
        """Close the connection."""
        if self._writer:
            self._writer.close()
            await self._writer.wait_closed()
            self._writer = None
            self._reader = None

    async def call(self, method: str, params: dict[str, Any] | None = None) -> Any:
        """Make a request to the daemon and return the result."""
        if not self._writer:
            await self.connect()

        request = {"method": method}
        if params:
            request["params"] = params

        self._writer.write((json.dumps(request) + "\n").encode())
        await self._writer.drain()

        line = await self._reader.readline()
        response = json.loads(line.decode())

        if not response.get("ok"):
            raise DaemonError(response.get("error", "Unknown error"))

        return response.get("result")

    async def subscribe(self, events: list[str]) -> AsyncIterator[dict]:
        """Subscribe to events and yield them as they arrive."""
        if not self._writer:
            await self.connect()

        await self.call("subscribe", {"events": events})

        while True:
            line = await self._reader.readline()
            if not line:
                break
            data = json.loads(line.decode())
            if "event" in data:
                yield data


class DaemonError(Exception):
    """Error from daemon."""
    pass


def is_daemon_running() -> bool:
    """Check if daemon is running by attempting to connect."""
    try:
        return asyncio.run(_check_daemon())
    except Exception:
        return False


async def _check_daemon() -> bool:
    client = DaemonClient()
    try:
        await client.call("status")
        return True
    except Exception:
        return False
    finally:
        await client.close()


# Fire-and-forget session logging (synchronous, non-blocking)


def _send_fire_and_forget(method: str, params: dict[str, Any]) -> None:
    """Send a request to lfd without waiting for response. Fails silently."""
    try:
        sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        sock.settimeout(0.5)
        sock.connect(str(SOCKET_PATH))
        request = json.dumps({"method": method, "params": params}) + "\n"
        sock.sendall(request.encode())
        sock.close()
    except Exception:
        pass  # Fire-and-forget: don't block on errors


def log_session_start(session: Session) -> None:
    """Tell lfd a session started. Fire-and-forget."""
    _send_fire_and_forget("sessions.start", {"session": session.to_dict()})


def log_session_end(session_id: str, status: SessionStatus) -> None:
    """Tell lfd a session ended. Fire-and-forget."""
    _send_fire_and_forget("sessions.end", {"session_id": session_id, "status": status.value})
