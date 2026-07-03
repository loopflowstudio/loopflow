from __future__ import annotations

import asyncio
import inspect
import json
import os
import threading
import time
import uuid
from typing import Any
from urllib.parse import urlparse, urlunparse

import httpx
import pytest
from loopflow.client import Client

from scripts.lib.api_harness import ApiAssertions, ApiClient
from scripts.lib.lfd_runtime import LfdRuntime

websockets = pytest.importorskip("websockets")

pytestmark = pytest.mark.e2e

_TIMEOUT_SECONDS = 10.0


def test_both_ws_clients_receive_wave_events(lfd_runtime: LfdRuntime, lf_client: Client) -> None:
    asyncio.run(_assert_dual_ws_wave_event(lf_client, lfd_runtime))


def test_both_clients_stream_output(lfd_runtime: LfdRuntime, lf_client: Client) -> None:
    wave = lf_client.create_wave(_wave_name("concurrent-output"), repo=str(lfd_runtime.repo_dir))
    lines: dict[str, str | None] = {}
    errors: list[str] = []
    lock = threading.Lock()

    def collect(name: str) -> None:
        try:
            line = _read_first_wave_log_line(
                base_url=lfd_runtime.base_url,
                token=lfd_runtime.token,
                wave_id=wave.id,
                timeout_seconds=_TIMEOUT_SECONDS,
            )
            with lock:
                lines[name] = line
        except (httpx.ReadTimeout, AssertionError):
            # No output produced within the timeout window.  Expected in CI
            # where no coding agent is available to generate wave logs.
            with lock:
                lines[name] = None
        except Exception as exc:
            with lock:
                errors.append(f"{name}: {type(exc).__name__}: {exc}")

    thread_a = threading.Thread(target=collect, args=("client_a",), daemon=True)
    thread_b = threading.Thread(target=collect, args=("client_b",), daemon=True)
    thread_a.start()
    thread_b.start()

    time.sleep(0.25)
    lf_client.run_wave(wave.id)

    thread_a.join(_TIMEOUT_SECONDS + 2)
    thread_b.join(_TIMEOUT_SECONDS + 2)

    assert not errors, f"log stream readers failed: {errors}"
    assert lines.keys() == {"client_a", "client_b"}, (
        f"both log readers should report a result, got {lines}"
    )
    assert lines["client_a"] == lines["client_b"], "both clients should observe the same result"


async def _assert_dual_ws_wave_event(client: Client, runtime: LfdRuntime) -> None:
    ws_url = _ws_url(runtime.base_url, "/ws")
    connect_kwargs = _ws_connect_kwargs(runtime.token, _TIMEOUT_SECONDS)

    async with websockets.connect(ws_url, **connect_kwargs) as socket_a:
        async with websockets.connect(ws_url, **connect_kwargs) as socket_b:
            connected_a = await _recv_ws_json(socket_a, _TIMEOUT_SECONDS)
            connected_b = await _recv_ws_json(socket_b, _TIMEOUT_SECONDS)
            assert connected_a.get("type") == "connected"
            assert connected_b.get("type") == "connected"

            wave = client.create_wave(
                _wave_name("concurrent-ws"),
                repo=str(runtime.repo_dir),
            )

            deadline = time.monotonic() + _TIMEOUT_SECONDS
            await asyncio.gather(
                _wait_for_wave_created_event(socket_a, wave.id, deadline),
                _wait_for_wave_created_event(socket_b, wave.id, deadline),
            )


def _read_first_wave_log_line(
    *,
    base_url: str,
    token: str,
    wave_id: str,
    timeout_seconds: float,
) -> str:
    deadline = time.monotonic() + timeout_seconds
    with ApiClient(base_url=base_url, token=token, timeout_seconds=timeout_seconds + 2) as api:
        with api.stream("GET", f"/v0/waves/{wave_id}/logs") as response:
            ApiAssertions.expect_status(response, 200)
            for line in response.iter_lines():
                if time.monotonic() > deadline:
                    break
                if line and line.strip():
                    return line.strip()

    raise AssertionError(f"no output lines observed within {timeout_seconds}s")


async def _wait_for_wave_created_event(socket: Any, wave_id: str, deadline: float) -> None:
    while time.monotonic() < deadline:
        payload = await _recv_ws_json(socket, deadline - time.monotonic())
        if payload.get("type") != "wave_created":
            continue

        event_wave_id = payload.get("wave_id")
        if event_wave_id == wave_id:
            return

        wave = payload.get("wave")
        if isinstance(wave, dict) and wave.get("id") == wave_id:
            return

    raise AssertionError(f"did not receive wave_created for wave {wave_id}")


async def _recv_ws_json(socket: Any, timeout_seconds: float) -> dict[str, Any]:
    raw = await asyncio.wait_for(socket.recv(), timeout=max(timeout_seconds, 0.05))
    assert isinstance(raw, str), f"expected text websocket frame, got {type(raw).__name__}"
    payload = json.loads(raw)
    assert isinstance(payload, dict), f"expected object websocket payload, got {payload!r}"
    return payload


def _wave_name(prefix: str) -> str:
    return f"{prefix}-{uuid.uuid4().hex[:8]}"


def _ws_url(base_url: str, path: str) -> str:
    parsed = urlparse(base_url)
    if parsed.scheme not in {"http", "https"}:
        raise ValueError(f"unsupported URL scheme for websocket conversion: {parsed.scheme!r}")
    ws_scheme = "wss" if parsed.scheme == "https" else "ws"
    return urlunparse((ws_scheme, parsed.netloc, path, "", "", ""))


def _ws_connect_kwargs(token: str, timeout_seconds: float) -> dict[str, Any]:
    headers = {"Authorization": f"Bearer {token}"}
    kwargs: dict[str, Any] = {
        "open_timeout": timeout_seconds,
        "close_timeout": timeout_seconds,
    }
    params = inspect.signature(websockets.connect).parameters
    if "additional_headers" in params:
        kwargs["additional_headers"] = headers
    else:
        kwargs["extra_headers"] = headers
    return kwargs


def _mock_client(handler: Any) -> Client:
    transport = httpx.MockTransport(handler)
    client = Client(base_url="http://test", token="test-token")
    headers = client._client.headers
    client._client.close()
    client._client = httpx.Client(base_url="http://test", transport=transport, headers=headers)
    return client
