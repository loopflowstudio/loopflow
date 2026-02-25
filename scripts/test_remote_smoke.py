#!/usr/bin/env python3
"""Remote lfd smoke checks over TLS reverse proxy."""

from __future__ import annotations

import argparse
import asyncio
import inspect
import json
import ssl
import sys
import time
import uuid
from pathlib import Path
from typing import Any, Callable
from urllib.parse import urlparse, urlunparse

import httpx

from lib.api_harness import ApiAssertions, ApiClient, ScenarioRunner

try:
    import websockets
except ImportError:  # pragma: no cover - runtime guard for script users
    websockets = None


class WebSocketClient:
    def __init__(
        self,
        base_url: str,
        token: str,
        timeout_seconds: float,
        ssl_context: ssl.SSLContext | None,
    ) -> None:
        self._base_url = base_url.rstrip("/")
        self._token = token
        self._timeout_seconds = timeout_seconds
        self._ssl_context = ssl_context

    def receive_connected_message(self) -> dict[str, Any]:
        return asyncio.run(self._receive_connected_message())

    async def _receive_connected_message(self) -> dict[str, Any]:
        if websockets is None:
            raise RuntimeError("Missing dependency: websockets. Run `uv sync --dev` and retry.")

        connect = websockets.connect
        header_field = (
            "additional_headers"
            if "additional_headers" in inspect.signature(connect).parameters
            else "extra_headers"
        )

        connect_kwargs: dict[str, Any] = {
            "open_timeout": self._timeout_seconds,
            "close_timeout": self._timeout_seconds,
            header_field: {"Authorization": f"Bearer {self._token}"},
        }
        if self._ssl_context is not None:
            connect_kwargs["ssl"] = self._ssl_context

        ws_url = _ws_url(self._base_url, "/ws")
        async with connect(ws_url, **connect_kwargs) as socket:
            raw = await asyncio.wait_for(socket.recv(), timeout=self._timeout_seconds)

        if not isinstance(raw, str):
            raise AssertionError("websocket first message was not text")

        try:
            payload = json.loads(raw)
        except json.JSONDecodeError as exc:
            raise AssertionError(f"websocket message was not JSON: {raw!r}") from exc

        if not isinstance(payload, dict):
            raise AssertionError(f"websocket payload must be object, got: {payload!r}")

        return payload


def main() -> int:
    args = _parse_args()
    verify, ssl_context = _resolve_tls(args)

    with ApiClient(
        base_url=args.url,
        token=args.token,
        timeout_seconds=args.timeout,
        verify=verify,
    ) as api, httpx.Client(
        base_url=args.url,
        headers={"Authorization": f"Bearer {args.token}"},
        timeout=args.timeout,
        verify=verify,
    ) as authed_stream:
        ws = WebSocketClient(
            base_url=args.url,
            token=args.token,
            timeout_seconds=args.timeout,
            ssl_context=ssl_context,
        )
        repo_path = _resolve_repo_path(api, args.repo)
        cleanup_waves: list[str] = []
        runner = ScenarioRunner()

        scenarios: list[tuple[str, Callable[[], None]]] = [
            ("health", lambda: _scenario_health(api)),
            ("wave_crud", lambda: _scenario_wave_crud(api, repo_path)),
            ("auth_rejection", lambda: _scenario_auth_rejection(api)),
            (
                "sse_streaming",
                lambda: _scenario_sse_streaming(
                    api,
                    authed_stream,
                    repo_path,
                    args.session_harness,
                    args.events_timeout,
                ),
            ),
            ("websocket_connected", lambda: _scenario_websocket(ws)),
            (
                "wave_run_and_logs",
                lambda: _scenario_wave_run_logs(
                    api,
                    authed_stream,
                    repo_path,
                    args.logs_timeout,
                    cleanup_waves,
                ),
            ),
            (
                "websocket_reconnect",
                lambda: _scenario_websocket_reconnect(api, ws, repo_path, cleanup_waves),
            ),
        ]

        try:
            for name, check in scenarios:
                runner.run_scenario(name, check)
            runner.print_summary()
        finally:
            _cleanup_waves(api, cleanup_waves)

    return 1 if runner.has_failures() else 0


def _scenario_health(api: ApiClient) -> None:
    response = api.request("GET", "/health")
    ApiAssertions.expect_status(response, 200)
    payload = ApiAssertions.expect_json_object(response)
    status = payload.get("status")
    if status != "ok":
        raise AssertionError(f"expected status='ok', got: {status!r}")


def _scenario_wave_crud(api: ApiClient, repo_path: str) -> None:
    wave_id = _create_wave(api, repo_path, "remote-smoke-crud")

    listed = api.request("GET", "/v0/waves")
    ApiAssertions.expect_status(listed, 200)
    list_payload = ApiAssertions.expect_json_object(listed)
    items = list_payload.get("data")
    if not isinstance(items, list):
        raise AssertionError(f"expected list response data, got: {list_payload!r}")
    if not any(isinstance(item, dict) and item.get("id") == wave_id for item in items):
        raise AssertionError(f"created wave {wave_id} missing from list")

    fetched = api.request("GET", f"/v0/waves/{wave_id}")
    ApiAssertions.expect_status(fetched, 200)
    fetched_payload = ApiAssertions.expect_json_object(fetched)
    if fetched_payload.get("id") != wave_id:
        raise AssertionError("fetched wave id mismatch")

    updated = api.request(
        "PATCH",
        f"/v0/waves/{wave_id}",
        json={"flow": "grind", "direction": ["infra"], "area": ["docs/"], "status": "paused"},
    )
    ApiAssertions.expect_status(updated, 200)
    updated_payload = ApiAssertions.expect_json_object(updated)
    if updated_payload.get("status") != "paused":
        raise AssertionError(f"expected paused status, got {updated_payload.get('status')!r}")

    deleted = api.request("DELETE", f"/v0/waves/{wave_id}")
    ApiAssertions.expect_status(deleted, 200)

    missing = api.request("GET", f"/v0/waves/{wave_id}")
    ApiAssertions.expect_error(missing, 404, message_contains="wave not found")


def _scenario_auth_rejection(api: ApiClient) -> None:
    response = api.request("GET", "/v0/waves", auth=False)
    ApiAssertions.expect_error(response, 401, message_contains="missing")


def _scenario_sse_streaming(
    api: ApiClient,
    authed_stream: httpx.Client,
    repo_path: str,
    session_harness: str,
    events_timeout: float,
) -> None:
    create_response = api.request(
        "POST",
        "/v0/sessions",
        json={
            "harness": session_harness,
            "step": "design",
            "repo_root": repo_path,
        },
    )
    ApiAssertions.expect_status(create_response, 200)
    session_payload = ApiAssertions.expect_json_object(create_response)
    session_id = _require_field(session_payload, "id")

    input_response = api.request(
        "POST",
        f"/v0/sessions/{session_id}/input",
        json={"content": "Reply with one short sentence."},
    )
    ApiAssertions.expect_status(input_response, 200)

    deadline = time.monotonic() + events_timeout
    seen_session_event = False
    current_event_name = ""

    with authed_stream.stream("GET", f"/v0/sessions/{session_id}/events") as response:
        ApiAssertions.expect_status(response, 200)
        for line in response.iter_lines():
            if time.monotonic() > deadline:
                break
            if not line:
                continue
            if line.startswith("event:"):
                current_event_name = line[6:].strip()
                continue
            if not line.startswith("data:") or current_event_name != "session.event":
                continue

            payload = line[5:].strip()
            if not payload:
                continue

            event = json.loads(payload)
            if isinstance(event, dict):
                seen_session_event = True
                break

    if not seen_session_event:
        raise AssertionError(f"no session.event received from SSE stream within {events_timeout}s")


def _scenario_websocket(ws: WebSocketClient) -> None:
    payload = ws.receive_connected_message()
    _expect_connected_message(payload)


def _scenario_wave_run_logs(
    api: ApiClient,
    authed_stream: httpx.Client,
    repo_path: str,
    logs_timeout: float,
    cleanup_waves: list[str],
) -> None:
    wave_id = _create_wave(api, repo_path, "remote-smoke-run")
    cleanup_waves.append(wave_id)

    run_response = api.request("POST", f"/v0/waves/{wave_id}/run", json={})
    ApiAssertions.expect_status(run_response, 200)
    run_payload = ApiAssertions.expect_json_object(run_response)
    if "started" not in run_payload:
        raise AssertionError(f"run response missing 'started': {run_payload!r}")

    deadline = time.monotonic() + logs_timeout
    saw_output_line = False

    with authed_stream.stream("GET", f"/v0/waves/{wave_id}/logs") as response:
        ApiAssertions.expect_status(response, 200)
        content_type = response.headers.get("content-type", "")
        if "text/plain" not in content_type:
            raise AssertionError(f"unexpected logs content-type: {content_type!r}")

        for line in response.iter_lines():
            if time.monotonic() > deadline:
                break
            if line and line.strip():
                saw_output_line = True
                break

    if not saw_output_line:
        raise AssertionError(f"no log output observed within {logs_timeout}s")


def _scenario_websocket_reconnect(
    api: ApiClient,
    ws: WebSocketClient,
    repo_path: str,
    cleanup_waves: list[str],
) -> None:
    first = ws.receive_connected_message()
    _expect_connected_message(first)

    new_wave_id = _create_wave(api, repo_path, "remote-smoke-reconnect")
    cleanup_waves.append(new_wave_id)

    second = ws.receive_connected_message()
    waves = _expect_connected_message(second)

    if not any(isinstance(wave, dict) and wave.get("id") == new_wave_id for wave in waves):
        raise AssertionError(f"reconnected websocket snapshot missing wave {new_wave_id}")


def _create_wave(api: ApiClient, repo_path: str, prefix: str) -> str:
    response = api.request(
        "POST",
        "/v0/waves",
        json={"repo": repo_path, "name": _wave_name(prefix)},
    )
    ApiAssertions.expect_status(response, 200)
    payload = ApiAssertions.expect_json_object(response)
    return _require_field(payload, "id")


def _expect_connected_message(payload: dict[str, Any]) -> list[Any]:
    if payload.get("type") != "connected":
        raise AssertionError(f"expected websocket type='connected', got: {payload!r}")

    waves = payload.get("waves")
    if not isinstance(waves, list):
        raise AssertionError(f"expected connected.waves list, got: {payload!r}")

    return waves


def _resolve_repo_path(api: ApiClient, repo_override: str | None) -> str:
    if repo_override:
        return repo_override

    response = api.request("GET", "/v0/repos")
    ApiAssertions.expect_status(response, 200)
    payload = ApiAssertions.expect_json_object(response)
    items = payload.get("data")
    if not isinstance(items, list) or not items:
        raise AssertionError(f"/v0/repos returned no repos: {payload!r}")

    first = items[0]
    if not isinstance(first, dict):
        raise AssertionError(f"invalid repo entry: {first!r}")

    path = first.get("path")
    if not isinstance(path, str) or not path:
        raise AssertionError(f"repo path missing in entry: {first!r}")

    return path


def _cleanup_waves(api: ApiClient, wave_ids: list[str]) -> None:
    for wave_id in wave_ids:
        try:
            api.request("DELETE", f"/v0/waves/{wave_id}")
        except Exception:
            continue


def _require_field(payload: dict[str, Any], name: str) -> str:
    value = payload.get(name)
    if not isinstance(value, str) or not value:
        raise AssertionError(f"missing required field {name!r}: {payload!r}")
    return value


def _wave_name(prefix: str) -> str:
    return f"{prefix}-{uuid.uuid4().hex[:8]}"


def _ws_url(base_url: str, path: str) -> str:
    parsed = urlparse(base_url)
    if parsed.scheme not in {"http", "https"}:
        raise ValueError(f"unsupported URL scheme for websocket conversion: {parsed.scheme!r}")

    ws_scheme = "wss" if parsed.scheme == "https" else "ws"
    return urlunparse((ws_scheme, parsed.netloc, path, "", "", ""))


def _resolve_tls(
    args: argparse.Namespace,
) -> tuple[bool | str, ssl.SSLContext | None]:
    if args.insecure and args.ca_cert:
        raise ValueError("use either --insecure or --ca-cert, not both")

    if args.insecure:
        context = ssl.create_default_context()
        context.check_hostname = False
        context.verify_mode = ssl.CERT_NONE
        return False, context

    if args.ca_cert:
        ca_path = Path(args.ca_cert).expanduser().resolve()
        if not ca_path.exists():
            raise ValueError(f"CA cert file does not exist: {ca_path}")
        context = ssl.create_default_context(cafile=str(ca_path))
        return str(ca_path), context

    return True, None


def _parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Run remote lfd smoke checks over TLS proxy")
    parser.add_argument("--url", required=True, help="Remote lfd base URL (https://lfd.example.com)")
    parser.add_argument("--token", required=True, help="Bearer token for remote auth")
    parser.add_argument(
        "--repo",
        default=None,
        help="Repo path for wave creation (defaults to first /v0/repos entry)",
    )
    parser.add_argument("--timeout", type=float, default=20.0, help="HTTP and websocket timeout in seconds")
    parser.add_argument("--events-timeout", type=float, default=30.0, help="Seconds to wait for a session SSE event")
    parser.add_argument("--logs-timeout", type=float, default=30.0, help="Seconds to wait for wave log output")
    parser.add_argument("--session-harness", default="claude", help="Harness for SSE session scenario")
    parser.add_argument("--insecure", action="store_true", help="Disable TLS certificate verification")
    parser.add_argument(
        "--ca-cert",
        default=None,
        help="Path to custom CA certificate for TLS verification",
    )
    return parser.parse_args()


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except KeyboardInterrupt:
        raise SystemExit(130)
    except ValueError as error:
        print(f"ERROR: {error}", file=sys.stderr)
        raise SystemExit(2)
