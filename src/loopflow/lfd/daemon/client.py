"""Client for connecting to lfd daemon."""

import asyncio
import json
from pathlib import Path
from typing import Any, AsyncIterator

from loopflow.lfd.step_run import SOCKET_PATH


class DaemonClient:
    """Client for connecting to lfd daemon from CLI or tests."""

    def __init__(self, socket_path: Path | None = None):
        self.socket_path = socket_path or SOCKET_PATH
        self._reader: asyncio.StreamReader | None = None
        self._writer: asyncio.StreamWriter | None = None

    async def connect(self) -> None:
        """Connect to the daemon socket."""
        self._reader, self._writer = await asyncio.open_unix_connection(str(self.socket_path))

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


def notify_event(event: str, data: dict[str, Any] | None = None) -> None:
    """Fire-and-forget event notification to daemon.

    If daemon is not running, silently ignores the notification.
    """
    try:
        asyncio.run(_notify_event(event, data or {}))
    except Exception:
        pass  # Fire-and-forget: ignore failures


async def _notify_event(event: str, data: dict[str, Any]) -> None:
    """Send event notification to daemon."""
    client = DaemonClient()
    try:
        await client.call("notify", {"event": event, "data": data})
    except Exception:
        pass  # Fire-and-forget
    finally:
        await client.close()
