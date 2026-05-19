#!/usr/bin/env python3
"""Pairing-token smoke test for QR/deep-link mobile setup."""

from __future__ import annotations

import argparse
import os
import re
import socket
import subprocess
import sys
import tempfile
import time
from pathlib import Path
from urllib.parse import parse_qs, urlparse

import httpx
from lib.api_harness import ApiAssertions, ApiClient, ScenarioRunner
from test_remote_smoke import WebSocketClient, _expect_connected_message

REPO_ROOT = Path(__file__).resolve().parents[1]


def main() -> int:
    args = _parse_args()
    runner = ScenarioRunner()

    _run_checked(["cargo", "build", "--bin", "lf", "--bin", "lfd"])

    with tempfile.TemporaryDirectory(prefix="lfd-pair-") as temp:
        temp_root = Path(temp)
        home = temp_root / "home"
        home.mkdir()
        port = _reserve_port()
        env = os.environ.copy()
        env["HOME"] = str(home)
        env["LFD_AUTH_MODE"] = "studio"
        env["LFD_HTTP_ADDR"] = f"127.0.0.1:{port}"
        env["GRPC_ENABLE_FORK_SUPPORT"] = "0"
        env["GRPC_VERBOSITY"] = "ERROR"

        log_path = temp_root / "lfd.log"
        with log_path.open("w", encoding="utf-8") as log:
            process = subprocess.Popen(
                [str(REPO_ROOT / "target" / "debug" / "lfd"), "serve"],
                cwd=REPO_ROOT,
                env=env,
                stdout=log,
                stderr=subprocess.STDOUT,
                text=True,
            )
            try:
                _wait_for_health(f"http://127.0.0.1:{port}", process, log_path, args.timeout)
                pair_url = _run_pair(env)
                payload = _parse_pair_url(pair_url)
                base_url = f"http://127.0.0.1:{port}"
                token = payload["token"]

                with ApiClient(base_url=base_url, token=token, timeout_seconds=args.timeout) as api:
                    ws = WebSocketClient(
                        base_url=base_url,
                        token=token,
                        timeout_seconds=args.timeout,
                        ssl_context=None,
                    )
                    runner.run_scenario("pair_url_shape", lambda: _scenario_pair_url(payload))
                    runner.run_scenario("paired_token_http", lambda: _scenario_http(api))
                    runner.run_scenario("paired_token_websocket", lambda: _scenario_ws(ws))
            finally:
                process.terminate()
                try:
                    process.wait(timeout=8)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait(timeout=5)

    runner.print_summary()
    return 1 if runner.has_failures() else 0


def _parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--timeout", type=float, default=10.0)
    return parser.parse_args()


def _run_pair(env: dict[str, str]) -> str:
    result = subprocess.run(
        [
            str(REPO_ROOT / "target" / "debug" / "lf"),
            "op",
            "pair",
            "--host",
            "100.64.1.2",
            "--no-tls",
        ],
        cwd=REPO_ROOT,
        env=env,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        raise RuntimeError(f"lf op pair failed\nstdout:\n{result.stdout}\nstderr:\n{result.stderr}")
    match = re.search(r"loopflow://pair\?\S+", result.stdout)
    if not match:
        raise RuntimeError(f"pair URL missing from output:\n{result.stdout}")
    return match.group(0)


def _parse_pair_url(url: str) -> dict[str, str]:
    parsed = urlparse(url)
    if parsed.scheme != "loopflow" or parsed.netloc != "pair":
        raise AssertionError(f"unexpected pair URL: {url}")
    query = parse_qs(parsed.query, strict_parsing=True)
    return {key: values[0] for key, values in query.items()}


def _scenario_pair_url(payload: dict[str, str]) -> None:
    assert payload["host"] == "100.64.1.2"
    assert payload["port"] == "2486"
    assert payload["tls"] == "false"
    assert payload["token"]


def _scenario_http(api: ApiClient) -> None:
    response = api.request("GET", "/v0/waves")
    ApiAssertions.expect_status(response, 200)
    payload = ApiAssertions.expect_json_object(response)
    if not isinstance(payload.get("data"), list):
        raise AssertionError(f"expected waves data list, got {payload!r}")


def _scenario_ws(ws: WebSocketClient) -> None:
    _expect_connected_message(ws.receive_connected_message())


def _wait_for_health(
    base_url: str,
    process: subprocess.Popen[str],
    log_path: Path,
    timeout: float,
) -> None:
    deadline = time.time() + timeout
    last_error = ""
    while time.time() < deadline:
        if process.poll() is not None:
            logs = log_path.read_text(encoding="utf-8")
            raise RuntimeError(f"lfd exited early\n{logs}")
        try:
            response = httpx.get(f"{base_url}/health", timeout=1.0)
            if response.status_code == 200:
                return
            last_error = response.text
        except httpx.HTTPError as exc:
            last_error = str(exc)
        time.sleep(0.2)
    logs = log_path.read_text(encoding="utf-8")
    raise RuntimeError(f"timed out waiting for lfd health: {last_error}\n{logs}")

def _reserve_port() -> int:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
        sock.bind(("127.0.0.1", 0))
        return sock.getsockname()[1]


def _run_checked(cmd: list[str]) -> None:
    result = subprocess.run(cmd, cwd=REPO_ROOT, capture_output=True, text=True)
    if result.returncode != 0:
        raise RuntimeError(f"command failed: {' '.join(cmd)}\n{result.stdout}\n{result.stderr}")


if __name__ == "__main__":
    sys.exit(main())
