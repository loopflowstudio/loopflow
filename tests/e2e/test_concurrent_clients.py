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
from loopflow.errors import LoopflowError
from scripts.lib.api_harness import ApiAssertions, ApiClient
from scripts.lib.lfd_runtime import LfdRuntime

websockets = pytest.importorskip("websockets")

pytestmark = pytest.mark.e2e

_TIMEOUT_SECONDS = 10.0
_INPUT_EVENT_TYPES = {
    "turn_started",
    "turn_completed",
    "item_started",
    "item_updated",
    "item_completed",
    "text_delta",
    "reasoning_delta",
    "diff_updated",
    "suggested_actions",
    "error",
}


def test_both_ws_clients_receive_wave_events(lfd_runtime: LfdRuntime) -> None:
    client = Client(base_url=lfd_runtime.base_url, token=lfd_runtime.token)
    try:
        asyncio.run(_assert_dual_ws_wave_event(client, lfd_runtime))
    finally:
        client.close()


def test_both_clients_stream_output(lfd_runtime: LfdRuntime) -> None:
    client = Client(base_url=lfd_runtime.base_url, token=lfd_runtime.token)
    try:
        wave = client.create_wave(_wave_name("concurrent-output"), repo=str(lfd_runtime.repo_dir))
        lines: dict[str, str] = {}
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
            except Exception as exc:
                with lock:
                    errors.append(f"{name}: {type(exc).__name__}: {exc}")

        thread_a = threading.Thread(target=collect, args=("client_a",), daemon=True)
        thread_b = threading.Thread(target=collect, args=("client_b",), daemon=True)
        thread_a.start()
        thread_b.start()

        time.sleep(0.25)
        client.run_wave(wave.id)

        thread_a.join(_TIMEOUT_SECONDS + 2)
        thread_b.join(_TIMEOUT_SECONDS + 2)

        assert not errors, f"log stream readers failed: {errors}"
        assert lines.keys() == {"client_a", "client_b"}, (
            f"both log readers should receive a line, got {lines}"
        )
        assert lines["client_a"] == lines["client_b"], (
            "both clients should observe the same first output line"
        )
    finally:
        client.close()


def test_both_clients_receive_session_events(lfd_runtime: LfdRuntime) -> None:
    _ensure_session_repo_root(lfd_runtime)
    session_id = _create_session(lfd_runtime)

    client_a = Client(base_url=lfd_runtime.base_url, token=lfd_runtime.token)
    client_b = Client(base_url=lfd_runtime.base_url, token=lfd_runtime.token)
    try:
        events_a: list[str] = []
        events_b: list[str] = []

        thread_a = threading.Thread(
            target=_collect_session_event_types,
            args=(client_a, session_id, events_a),
            daemon=True,
        )
        thread_b = threading.Thread(
            target=_collect_session_event_types,
            args=(client_b, session_id, events_b),
            daemon=True,
        )
        thread_a.start()
        thread_b.start()
        thread_a.join(_TIMEOUT_SECONDS + 2)
        thread_b.join(_TIMEOUT_SECONDS + 2)

        assert events_a, "client A should receive at least one session event"
        assert events_b, "client B should receive at least one session event"
        assert events_a[0] == "status_changed", f"unexpected first event: {events_a[0]}"
        assert events_b[0] == "status_changed", f"unexpected first event: {events_b[0]}"
    finally:
        _stop_session(lfd_runtime, session_id)
        client_a.close()
        client_b.close()


def test_chat_input_from_either_client_visible_to_both(lfd_runtime: LfdRuntime) -> None:
    _ensure_session_repo_root(lfd_runtime)
    session_id = _create_session(lfd_runtime)

    client_a = Client(base_url=lfd_runtime.base_url, token=lfd_runtime.token)
    client_b = Client(base_url=lfd_runtime.base_url, token=lfd_runtime.token)
    try:
        events_a: list[str] = []
        events_b: list[str] = []

        thread_a = threading.Thread(
            target=_collect_session_event_types,
            args=(client_a, session_id, events_a),
            kwargs={"max_events": 20, "timeout_seconds": _TIMEOUT_SECONDS},
            daemon=True,
        )
        thread_b = threading.Thread(
            target=_collect_session_event_types,
            args=(client_b, session_id, events_b),
            kwargs={"max_events": 20, "timeout_seconds": _TIMEOUT_SECONDS},
            daemon=True,
        )
        thread_a.start()
        thread_b.start()

        status = _wait_for_session_status(client_a, session_id, timeout_seconds=4)
        send_succeeded = False
        send_error = ""
        try:
            client_a.send_session_input(session_id, "Reply with one short sentence.")
            send_succeeded = True
        except LoopflowError as exc:
            send_error = str(exc)

        thread_a.join(_TIMEOUT_SECONDS + 2)
        thread_b.join(_TIMEOUT_SECONDS + 2)

        assert events_a, "client A should receive session events"
        assert events_b, "client B should receive session events"
        assert "status_changed" in events_a
        assert "status_changed" in events_b

        saw_input_events_a = any(event in _INPUT_EVENT_TYPES for event in events_a)
        saw_input_events_b = any(event in _INPUT_EVENT_TYPES for event in events_b)
        if saw_input_events_a or saw_input_events_b:
            assert saw_input_events_a, f"client A missed input events: {events_a}"
            assert saw_input_events_b, f"client B missed input events: {events_b}"
        elif not send_succeeded:
            assert send_error, "failed send should provide an error"
            assert status in {"starting", "failed", "ended", "active"}
        else:
            assert status in {"active", "failed", "ended"}
    finally:
        _stop_session(lfd_runtime, session_id)
        client_a.close()
        client_b.close()


def test_suggested_actions_event_type_is_parseable() -> None:
    def handler(_request: httpx.Request) -> httpx.Response:
        return httpx.Response(
            200,
            text='\n'.join([
                "id: 7",
                (
                    'data: {"type":"suggested_actions","turn_id":"turn_1",'
                    '"actions":[{"label":"Try again"}]}'
                ),
                "",
            ]),
        )

    client = _mock_client(handler)
    try:
        events = list(client.stream_session_events("session-1", timeout=1))
    finally:
        client.close()

    assert len(events) == 1
    assert events[0].event["type"] == "suggested_actions"


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


def _create_session(runtime: LfdRuntime) -> str:
    harness = _session_harness()
    with ApiClient(base_url=runtime.base_url, token=runtime.token) as api:
        response = api.request(
            "POST",
            "/v0/sessions",
            json={
                "harness": harness,
                "step": "design",
                "repo_root": str(runtime.repo_dir),
            },
        )
        ApiAssertions.expect_status(response, 200)
        payload = ApiAssertions.expect_json_object(response)

    session_id = payload.get("id")
    assert isinstance(session_id, str) and session_id, f"invalid session payload: {payload}"
    return session_id


def _stop_session(runtime: LfdRuntime, session_id: str) -> None:
    with ApiClient(base_url=runtime.base_url, token=runtime.token) as api:
        api.request("DELETE", f"/v0/sessions/{session_id}")


def _collect_session_event_types(
    client: Client,
    session_id: str,
    sink: list[str],
    *,
    max_events: int = 6,
    timeout_seconds: float = _TIMEOUT_SECONDS,
) -> None:
    deadline = time.monotonic() + timeout_seconds
    try:
        for envelope in client.stream_session_events(session_id, timeout=timeout_seconds):
            event_type = envelope.event.get("type")
            if isinstance(event_type, str):
                sink.append(event_type)
                if len(sink) >= max_events:
                    break
            if time.monotonic() > deadline:
                break
    except (ConnectionError, LoopflowError):
        return


def _wait_for_session_status(client: Client, session_id: str, timeout_seconds: float) -> str:
    deadline = time.monotonic() + timeout_seconds
    last_status = "starting"
    while time.monotonic() < deadline:
        session = client.session(session_id)
        if session is None:
            return "ended"
        last_status = session.status
        if last_status in {"active", "failed", "ended"}:
            return last_status
        time.sleep(0.1)
    return last_status


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


def _session_harness() -> str:
    return os.environ.get("LOOPFLOW_E2E_SESSION_HARNESS", "codex")


def _ensure_session_repo_root(runtime: LfdRuntime) -> None:
    (runtime.repo_dir / ".lf").mkdir(parents=True, exist_ok=True)


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
