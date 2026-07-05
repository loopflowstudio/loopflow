#!/usr/bin/env python3
"""Remote lfd smoke checks over TLS reverse proxy."""

from __future__ import annotations

import argparse
import asyncio
import inspect
import json
import ssl
import sys
from pathlib import Path
from typing import Any, Callable
from urllib.parse import urlparse, urlunparse

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
    ) as api:
        ws = WebSocketClient(
            base_url=args.url,
            token=args.token,
            timeout_seconds=args.timeout,
            ssl_context=ssl_context,
        )
        runner = ScenarioRunner()

        # Read-only checks: the gatekeeper serves reads and relay; wave
        # creation is writing wave/<name>/ markdown in the repo, not a POST.
        scenarios: list[tuple[str, Callable[[], None]]] = [
            ("health", lambda: _scenario_health(api)),
            ("waves_list", lambda: _scenario_waves_list(api)),
            ("auth_rejection", lambda: _scenario_auth_rejection(api)),
            ("websocket_connected", lambda: _scenario_websocket(ws)),
        ]

        for name, check in scenarios:
            runner.run_scenario(name, check)
        runner.print_summary()

    return 1 if runner.has_failures() else 0


def _scenario_health(api: ApiClient) -> None:
    response = api.request("GET", "/health")
    ApiAssertions.expect_status(response, 200)
    payload = ApiAssertions.expect_json_object(response)
    status = payload.get("status")
    if status != "ok":
        raise AssertionError(f"expected status='ok', got: {status!r}")


def _scenario_waves_list(api: ApiClient) -> None:
    listed = api.request("GET", "/v0/waves")
    ApiAssertions.expect_status(listed, 200)
    list_payload = ApiAssertions.expect_json_object(listed)
    items = list_payload.get("data")
    if not isinstance(items, list):
        raise AssertionError(f"expected list response data, got: {list_payload!r}")

    for item in items[:1]:
        wave_id = item.get("id") if isinstance(item, dict) else None
        if not wave_id:
            continue
        fetched = api.request("GET", f"/v0/waves/{wave_id}")
        ApiAssertions.expect_status(fetched, 200)
        fetched_payload = ApiAssertions.expect_json_object(fetched)
        if fetched_payload.get("id") != wave_id:
            raise AssertionError("fetched wave id mismatch")


def _scenario_auth_rejection(api: ApiClient) -> None:
    response = api.request("GET", "/v0/waves", auth=False)
    ApiAssertions.expect_error(response, 401, message_contains="missing")


def _scenario_websocket(ws: WebSocketClient) -> None:
    payload = ws.receive_connected_message()
    _expect_connected_message(payload)


def _expect_connected_message(payload: dict[str, Any]) -> list[Any]:
    if payload.get("type") != "connected":
        raise AssertionError(f"expected websocket type='connected', got: {payload!r}")

    waves = payload.get("waves")
    if not isinstance(waves, list):
        raise AssertionError(f"expected connected.waves list, got: {payload!r}")

    return waves


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
    parser.add_argument(
        "--url",
        required=True,
        help="Remote lfd base URL (https://lfd.example.com)",
    )
    parser.add_argument("--token", required=True, help="Bearer token for remote auth")
    parser.add_argument(
        "--timeout",
        type=float,
        default=20.0,
        help="HTTP and websocket timeout in seconds",
    )
    parser.add_argument(
        "--insecure",
        action="store_true",
        help="Disable TLS certificate verification",
    )
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
