from __future__ import annotations

import asyncio
import json
import logging
import time
import urllib.error
import urllib.request
from dataclasses import dataclass
from typing import Any


@dataclass
class RegistrationState:
    enabled: bool = False
    registered: bool = False
    connection_token: str | None = None
    expires_at: float | None = None
    last_error: str | None = None
    last_heartbeat: float | None = None
    machine_id: str | None = None
    machine_name: str | None = None

    def to_status(self) -> dict[str, Any]:
        return {
            "enabled": self.enabled,
            "registered": self.registered,
            "expires_at": self.expires_at,
            "last_error": self.last_error,
            "last_heartbeat": self.last_heartbeat,
            "machine_id": self.machine_id,
            "machine_name": self.machine_name,
        }


_state = RegistrationState()


def get_registration_status() -> dict[str, Any]:
    return _state.to_status()


def set_registration_enabled(enabled: bool) -> None:
    _state.enabled = enabled


def set_registration_error(message: str | None) -> None:
    _state.last_error = message


class RegistrationClient:
    def __init__(self, base_url: str = "https://loopflow.studio"):
        self.base_url = base_url.rstrip("/")
        self.state = _state
        self._heartbeat_task: asyncio.Task | None = None
        self._logger = logging.getLogger("lfd.registration")

    async def register(self, jwt: str, machine_id: str, machine_name: str) -> str:
        """Register daemon with loopflow.studio. Returns connection token."""
        payload = {
            "machine_id": machine_id,
            "machine_name": machine_name,
            "capabilities": ["waves", "terminal", "grpc"],
            "grpc_port": 50051,
        }
        headers = {"Authorization": f"Bearer {jwt}"}
        status, data = await _post_json(
            f"{self.base_url}/api/v1/daemons/register", payload, headers=headers
        )

        if status != 200:
            raise RuntimeError(f"registration failed: HTTP {status}")
        if not isinstance(data, dict):
            raise RuntimeError("registration failed: invalid response")
        token = data.get("connection_token")
        expires = data.get("expires_at")
        if not isinstance(token, str) or not token:
            raise RuntimeError("registration failed: missing token")

        self.state.enabled = True
        self.state.registered = True
        self.state.connection_token = token
        self.state.expires_at = float(expires) if isinstance(expires, (int, float)) else None
        self.state.last_error = None
        self.state.machine_id = machine_id
        self.state.machine_name = machine_name
        return token

    async def start_heartbeat(self, jwt: str, machine_id: str, interval: float = 30.0) -> None:
        """Start background heartbeat task."""

        async def heartbeat_loop() -> None:
            while True:
                await asyncio.sleep(interval)
                try:
                    await self._send_heartbeat(jwt, machine_id)
                except Exception as e:
                    self.state.last_error = str(e)
                    self._logger.warning("Registration heartbeat failed: %s", e)

        if self._heartbeat_task and not self._heartbeat_task.done():
            return

        self._heartbeat_task = asyncio.create_task(heartbeat_loop())

    async def _send_heartbeat(self, jwt: str, machine_id: str) -> None:
        headers = {"Authorization": f"Bearer {jwt}"}
        status, _ = await _post_json(
            f"{self.base_url}/api/v1/daemons/heartbeat",
            {"machine_id": machine_id},
            headers=headers,
        )
        if status != 200:
            raise RuntimeError(f"heartbeat failed: HTTP {status}")
        self.state.last_heartbeat = time.time()

    async def deregister(self, jwt: str, machine_id: str) -> None:
        """Deregister on shutdown."""
        if self._heartbeat_task:
            self._heartbeat_task.cancel()

        if self.state.registered:
            headers = {"Authorization": f"Bearer {jwt}"}
            try:
                await _post_json(
                    f"{self.base_url}/api/v1/daemons/deregister",
                    {"machine_id": machine_id},
                    headers=headers,
                )
            except Exception:
                pass

        self.state.registered = False
        self.state.connection_token = None
        self.state.expires_at = None


async def _post_json(
    url: str,
    payload: dict[str, Any],
    headers: dict[str, str] | None = None,
    timeout: int = 5,
) -> tuple[int, dict[str, Any] | None]:
    return await asyncio.to_thread(_post_json_sync, url, payload, headers, timeout)


def _post_json_sync(
    url: str,
    payload: dict[str, Any],
    headers: dict[str, str] | None,
    timeout: int,
) -> tuple[int, dict[str, Any] | None]:
    data = json.dumps(payload).encode("utf-8")
    req = urllib.request.Request(url, data=data, method="POST")
    req.add_header("Content-Type", "application/json")
    req.add_header("Accept", "application/json")
    if headers:
        for key, value in headers.items():
            req.add_header(key, value)

    try:
        with urllib.request.urlopen(req, timeout=timeout) as resp:
            status = resp.status
            body = resp.read().decode("utf-8")
    except urllib.error.HTTPError as e:
        status = e.code
        body = e.read().decode("utf-8") if e.fp else ""
    except Exception:
        return 0, None

    try:
        parsed = json.loads(body) if body else None
    except Exception:
        parsed = None
    return status, parsed if isinstance(parsed, dict) else None
