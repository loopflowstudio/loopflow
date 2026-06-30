#!/usr/bin/env python3
"""Verify lfd-owned embedded terminal lifecycle.

Starts lfd, creates a throwaway wave for the current repo, launches a palette
terminal session through POST /v0/terminal-sessions, and proves the tmux session
stays attachable after the lf command exits.

Usage:
    uv run python scripts/verify_embedded_build_driver.py
    uv run python scripts/verify_embedded_build_driver.py --skip-build --flow ship --agent codex
"""

from __future__ import annotations

import argparse
import json
import os
import signal
import subprocess
import time
import urllib.error
import urllib.request
from pathlib import Path
from uuid import uuid4

REPO_ROOT = Path(__file__).resolve().parent.parent
LFD_BIN = REPO_ROOT / "target" / "debug" / "lfd"
ROOT_URL = "http://127.0.0.1:2486"
BASE_URL = f"{ROOT_URL}/v0"


def log(message: str) -> None:
    print(message, flush=True)


def fail(message: str) -> None:
    log(f"FAIL: {message}")
    raise SystemExit(1)


def run(cmd: list[str], **kwargs) -> subprocess.CompletedProcess:
    return subprocess.run(cmd, text=True, **kwargs)


def read_token() -> str | None:
    try:
        return (Path.home() / ".lf" / "session-token").read_text().strip() or None
    except OSError:
        return None


def request(method: str, path: str, token: str, body: dict | None = None) -> dict:
    data = None
    headers = {"Authorization": f"Bearer {token}"}
    if body is not None:
        data = json.dumps(body).encode()
        headers["Content-Type"] = "application/json"
    req = urllib.request.Request(f"{BASE_URL}{path}", data=data, headers=headers, method=method)
    try:
        with urllib.request.urlopen(req, timeout=10) as response:
            payload = response.read().decode()
    except urllib.error.HTTPError as error:
        detail = error.read().decode(errors="replace")
        fail(f"{method} {path} failed: HTTP {error.code}\n{detail}")
    return json.loads(payload) if payload else {}


def health_ready() -> bool:
    try:
        with urllib.request.urlopen(f"{ROOT_URL}/health", timeout=2) as response:
            return response.status == 200
    except urllib.error.URLError:
        return False


def build_lfd() -> None:
    log("Building lfd…")
    result = run(["cargo", "build", "--bin", "lfd"], cwd=REPO_ROOT)
    if result.returncode != 0:
        fail("cargo build --bin lfd failed")


def kill_existing_lfd() -> None:
    result = run(["lsof", "-ti", ":2486"], capture_output=True)
    if result.returncode != 0:
        return
    for pid in result.stdout.splitlines():
        try:
            os.kill(int(pid), signal.SIGTERM)
        except (ValueError, ProcessLookupError):
            pass
    time.sleep(1)


def start_lfd() -> subprocess.Popen:
    kill_existing_lfd()
    env = os.environ.copy()
    env["RUST_LOG"] = "loopflow=info"
    # Self-contained smoke: this script exercises terminal-session lifecycle,
    # not auth wiring. Pin an explicit bearer token so host config cannot leak
    # into the smoke run.
    env["LFD_AUTH_TOKEN"] = "embedded-smoke-token"
    proc = subprocess.Popen(
        [str(LFD_BIN), "serve"],
        cwd=REPO_ROOT,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
        env=env,
    )
    for _ in range(40):
        time.sleep(0.5)
        token = read_token()
        if token and health_ready():
            log("lfd ready")
            return proc
    proc.terminate()
    try:
        output, _ = proc.communicate(timeout=5)
    except subprocess.TimeoutExpired:
        proc.kill()
        output, _ = proc.communicate(timeout=5)
    fail(f"lfd did not become ready\n{output[-2000:]}")


def tmux_has_session(name: str) -> bool:
    return run(["tmux", "has-session", "-t", name], capture_output=True).returncode == 0


def wait_for_terminal_status(token: str, session_id: str, timeout: float = 30) -> dict:
    deadline = time.time() + timeout
    last = None
    while time.time() < deadline:
        last = request("GET", f"/terminal-sessions/{session_id}", token)
        if last["status"] in {"succeeded", "failed", "canceled"}:
            return last
        time.sleep(0.5)
    fail(f"terminal session did not reach terminal status: {last}")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--skip-build", action="store_true")
    parser.add_argument("--repo", default=str(REPO_ROOT))
    parser.add_argument("--worktree", default=str(REPO_ROOT))
    parser.add_argument("--flow", default="__palette_smoke_missing_step__")
    parser.add_argument("--agent", default="claude:opus")
    args = parser.parse_args()

    if not args.skip_build:
        build_lfd()
    proc = start_lfd()
    try:
        token = read_token()
        if not token:
            fail("missing lfd session token")
        wave = request(
            "POST",
            "/waves",
            token,
            {
                "repo": args.repo,
                "name": f"palette-smoke-{uuid4().hex[:8]}",
                "flow": args.flow,
                "workers": 0,
                "run": False,
            },
        )
        created = request(
            "POST",
            "/terminal-sessions",
            token,
            {
                "wave_id": wave["id"],
                "flow": args.flow,
                "worktree": args.worktree,
                "agent": args.agent,
            },
        )
        session = created["session"]
        connection = created["connection"]
        assert session["source"] == "palette"
        assert session["agent"] == args.agent
        assert connection["session_name"] == session["tmux_name"]
        if not tmux_has_session(session["tmux_name"]):
            fail("tmux session was not created")
        completed = wait_for_terminal_status(token, session["id"])
        if not tmux_has_session(session["tmux_name"]):
            fail("palette tmux session exited instead of staying attachable")
        log(f"OK: {completed['status']} session stayed attachable at {session['tmux_name']}")
        request("POST", f"/terminal-sessions/{session['id']}/cancel", token, {})
    finally:
        proc.terminate()
        try:
            proc.wait(timeout=5)
        except subprocess.TimeoutExpired:
            proc.kill()


if __name__ == "__main__":
    main()
