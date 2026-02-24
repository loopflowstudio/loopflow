#!/usr/bin/env python3
"""Shared fork execution helpers for script and pytest runners."""

from __future__ import annotations

import os
import re
import select
import signal
import subprocess
import shutil
import time
import uuid
from pathlib import Path
from typing import Callable, TypeVar

from loopflow.client import Client

REPO_ROOT = Path(__file__).resolve().parents[2]
LFD_BIN = REPO_ROOT / "target" / "debug" / "lfd"
PG_CONTAINER = "lfd-dev-postgres"
WAVE_NAME = "fork-test"
_SESSION_TOKEN: str | None = None
T = TypeVar("T")


def ensure_postgres() -> None:
    result = _run(
        ["docker", "inspect", PG_CONTAINER, "--format", "{{.State.Running}}"],
        capture_output=True,
    )
    if result.returncode == 0 and result.stdout.strip() == "true":
        return

    _run(["docker", "rm", "-f", PG_CONTAINER], capture_output=True)
    result = _run(
        [
            "docker",
            "run",
            "-d",
            "--name",
            PG_CONTAINER,
            "-p",
            "5432:5432",
            "-e",
            "POSTGRES_USER=lfd",
            "-e",
            "POSTGRES_PASSWORD=lfd",
            "-e",
            "POSTGRES_DB=lfd",
            "--health-cmd",
            "pg_isready -U lfd",
            "--health-interval",
            "2s",
            "--health-retries",
            "10",
            "postgres:16-alpine",
        ]
    )
    if result.returncode != 0:
        raise RuntimeError("failed to start postgres")

    print("Waiting for postgres...")
    for _ in range(30):
        time.sleep(1)
        check = _run(
            ["docker", "inspect", PG_CONTAINER, "--format", "{{.State.Health.Status}}"],
            capture_output=True,
        )
        if check.stdout.strip() == "healthy":
            return

    raise RuntimeError("postgres did not become healthy")


def ensure_agent_image() -> None:
    result = _run(["docker", "image", "inspect", "loopflow/agent:latest"], capture_output=True)
    if result.returncode == 0:
        return

    print("Building agent image...")
    result = _run(["docker", "build", "-t", "loopflow/agent:latest", "docker/agent"], cwd=REPO_ROOT)
    if result.returncode != 0:
        raise RuntimeError("failed to build agent image")


def build_lfd() -> None:
    print("Building lfd...")
    result = _run(["cargo", "build", "--bin", "lfd"], cwd=REPO_ROOT)
    if result.returncode != 0:
        raise RuntimeError("cargo build failed")


def kill_existing_lfd() -> None:
    result = _run(["lsof", "-ti", ":2486"], capture_output=True)
    if result.returncode != 0 or not result.stdout.strip():
        return

    for pid in result.stdout.strip().splitlines():
        try:
            os.kill(int(pid), signal.SIGTERM)
        except (ValueError, ProcessLookupError):
            continue
    time.sleep(1)


def start_lfd_container_mode() -> subprocess.Popen[str]:
    global _SESSION_TOKEN

    kill_existing_lfd()
    _SESSION_TOKEN = None
    previous_token = _read_session_token()

    env = os.environ.copy()
    env["LFD_MODE"] = "container"
    env["LFD_DATABASE_URL"] = "postgres://lfd:lfd@127.0.0.1:5432/lfd"
    env["LFD_EXECUTOR_CREDENTIALS_MOUNTS"] = "claude,ssh,gitconfig"
    env["RUST_LOG"] = "loopflow=debug,tower_http=debug"
    env["GRPC_ENABLE_FORK_SUPPORT"] = "0"
    env["GRPC_VERBOSITY"] = "ERROR"

    process = subprocess.Popen(
        [str(LFD_BIN), "serve"],
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
        bufsize=1,
        env=env,
    )

    for _ in range(120):
        time.sleep(0.5)
        check = _run(
            ["curl", "-sf", "--max-time", "1", "http://127.0.0.1:2486/health"],
            capture_output=True,
        )
        if check.returncode == 0:
            _SESSION_TOKEN = _wait_for_session_token(previous=previous_token)
            print("lfd ready")
            return process

    stop_process(process)
    stdout = process.stdout.read() if process.stdout else ""
    raise RuntimeError(f"lfd did not become ready.\nOutput:\n{stdout[:2000]}")


def create_and_run_wave(flow: str, direction: str, wave_name: str | None = None) -> str:
    selected_wave_name = wave_name or f"{WAVE_NAME}-{uuid.uuid4().hex[:8]}"
    repo = str(REPO_ROOT.resolve())
    client = Client(timeout=30.0, token=_current_token())

    try:
        try:
            client.delete_wave(selected_wave_name)
        except Exception:
            pass

        _retry(
            lambda: client.create_wave(
                selected_wave_name, repo=repo, flow=flow, direction=[direction]
            )
        )
        _retry(lambda: client.run_wave(selected_wave_name))
        print(f"Wave '{selected_wave_name}' started with flow '{flow}'")
    finally:
        client.close()

    return selected_wave_name


def create_test_wave_name(prefix: str = WAVE_NAME) -> str:
    return f"{prefix}-{uuid.uuid4().hex[:8]}"


def cleanup_wave_artifacts(wave_name: str) -> None:
    """Best-effort cleanup for fork test waves and worktrees."""
    client = Client(timeout=10.0, token=_current_token())
    try:
        try:
            client.stop_wave(wave_name)
        except Exception:
            pass

        try:
            client.delete_wave(wave_name)
        except Exception:
            pass
    finally:
        client.close()

    _cleanup_local_worktrees(wave_name)


def has_claude_credentials() -> bool:
    claude_dir = Path.home() / ".claude"
    has_oauth = claude_dir.exists() and any(claude_dir.iterdir())
    has_api_key = bool(os.environ.get("ANTHROPIC_API_KEY"))
    return has_oauth or has_api_key


def stop_process(process: subprocess.Popen[str]) -> None:
    if process.poll() is not None:
        return

    process.terminate()
    try:
        process.wait(timeout=5)
    except subprocess.TimeoutExpired:
        process.kill()


def wait_for_completion(
    process: subprocess.Popen[str], timeout: int, wave_name: str | None = None
) -> tuple[bool, str]:
    if process.stdout is None:
        raise RuntimeError("lfd stdout is not available")

    status_client = Client(timeout=5.0, token=_current_token())
    try:
        lines: list[str] = []
        deadline = time.time() + timeout
        last_status_check = 0.0
        fork_errors: list[str] = []
        completed_branches = 0
        total_branches = 0

        while time.time() < deadline:
            now = time.time()
            if wave_name and now - last_status_check >= 2:
                last_status_check = now
                try:
                    wave = status_client.wave(wave_name)
                    if wave is not None:
                        status = wave.status.lower()
                        if status in {"completed", "failed", "stopped"}:
                            output = "".join(lines) + f"\n[WAVE_STATUS={status}]"
                            return status == "completed", output
                except Exception:
                    pass

            if process.poll() is not None:
                lines.append(process.stdout.read())
                output = "".join(lines) + f"\n[PROCESS_EXITED={process.returncode}]"
                return False, output

            line = process.stdout.readline()
            if not line:
                time.sleep(0.1)
                continue

            lines.append(line)
            stripped = line.strip()

            if any(
                keyword in stripped
                for keyword in [
                    "INFO",
                    "WARN",
                    "ERROR",
                    "fork branch",
                    "creating agent",
                    "running fork",
                    "synthesize",
                ]
            ):
                print(f"  {stripped}")

            if "total_branches=" in stripped:
                match = re.search(r"total_branches=(\d+)", stripped)
                if match:
                    total_branches = int(match.group(1))

            if "fork branch error" in stripped:
                fork_errors.append(stripped)

            if "fork branch done" in stripped:
                match = re.search(r"completed=(\d+)", stripped)
                if match:
                    completed_branches = int(match.group(1))

            if total_branches > 0 and completed_branches >= total_branches and fork_errors:
                return False, "".join(lines) + "\n[FORK_BRANCH_ERRORS]"

            if "synthesize" in stripped and "completed" in stripped.lower():
                return True, "".join(lines)

            if "wave run completed" in stripped or "wave run failed" in stripped:
                outcome = "completed" if "completed" in stripped else "failed"
                return "completed" in stripped, "".join(lines) + f"\n[WAVE_LOG_STATUS={outcome}]"

        return False, "".join(lines) + "\n[TIMEOUT]"
    finally:
        status_client.close()


def wait_for_fork_launch(
    process: subprocess.Popen[str], timeout: int, wave_name: str | None = None
) -> tuple[bool, str]:
    if process.stdout is None:
        raise RuntimeError("lfd stdout is not available")

    lines: list[str] = []
    deadline = time.time() + timeout

    while time.time() < deadline:
        if process.poll() is not None:
            lines.append(process.stdout.read())
            output = "".join(lines) + f"\n[PROCESS_EXITED={process.returncode}]"
            return False, output

        if wave_name and _fork_worktrees_created(wave_name):
            return True, "".join(lines) + "\n[FORK_WORKTREE_CREATED]"

        ready, _, _ = select.select([process.stdout], [], [], 0.2)
        if not ready:
            continue

        line = process.stdout.readline()
        if not line:
            continue

        lines.append(line)
        stripped = line.strip()
        if "launching fork branch agent" in stripped:
            return True, "".join(lines) + "\n[FORK_AGENT_LAUNCHED]"
        if "creating agent container" in stripped and (
            wave_name is None or wave_name in stripped
        ):
            return True, "".join(lines) + "\n[FORK_CONTAINER_CREATED]"
        if "fork branch completed" in stripped or "fork branch failed" in stripped:
            return True, "".join(lines) + "\n[FORK_BRANCH_FINISHED]"

    return False, "".join(lines) + "\n[TIMEOUT]"


def _run(cmd: list[str], capture_output: bool = False, **kwargs) -> subprocess.CompletedProcess[str]:
    return subprocess.run(cmd, capture_output=capture_output, text=True, **kwargs)


def _retry(call: Callable[[], T], timeout: float = 60.0, delay: float = 1.0) -> T:
    deadline = time.time() + timeout
    while True:
        try:
            return call()
        except ConnectionError:
            if time.time() >= deadline:
                raise
            time.sleep(delay)


def _read_session_token() -> str | None:
    token_path = Path.home() / ".lf" / "session-token"
    if not token_path.exists():
        return None
    token = token_path.read_text(encoding="utf-8").strip()
    return token or None


def _current_token() -> str:
    if _SESSION_TOKEN:
        return _SESSION_TOKEN
    return _wait_for_session_token()


def _wait_for_session_token(timeout: float = 15.0, previous: str | None = None) -> str:
    token_path = Path.home() / ".lf" / "session-token"
    deadline = time.time() + timeout
    while time.time() < deadline:
        token = _read_session_token()
        if token is not None:
            if previous is None or token != previous:
                return token
        time.sleep(0.1)
    raise RuntimeError(f"missing session token at {token_path}")


def _fork_worktrees_created(wave_name: str) -> bool:
    repo_root = _main_repo_root(REPO_ROOT)
    parent = repo_root.parent or repo_root
    pattern = f"{repo_root.name}.{wave_name}-fork-*"
    return any(path.is_dir() for path in parent.glob(pattern))


def _main_repo_root(repo: Path) -> Path:
    result = _run(
        ["git", "-C", str(repo), "rev-parse", "--path-format=absolute", "--git-common-dir"],
        capture_output=True,
    )
    if result.returncode == 0:
        common_dir = Path(result.stdout.strip())
        if common_dir.name == ".git":
            return common_dir.parent
    return repo


def _cleanup_local_worktrees(wave_name: str) -> None:
    repo_root = REPO_ROOT.resolve()
    parent = repo_root.parent
    base = parent / f"{repo_root.name}.{wave_name}"
    candidates = [base] + sorted(parent.glob(f"{base.name}-fork-*"))

    for worktree in candidates:
        if not worktree.exists():
            continue
        _run(
            ["git", "-C", str(repo_root), "worktree", "remove", "--force", str(worktree)],
            capture_output=True,
        )
        if worktree.exists():
            shutil.rmtree(worktree, ignore_errors=True)
