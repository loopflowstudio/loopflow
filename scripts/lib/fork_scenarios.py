#!/usr/bin/env python3
"""Shared fork execution helpers for script and pytest runners."""

from __future__ import annotations

import os
import re
import signal
import subprocess
import time
from pathlib import Path

import loopflow.api as loopflow_api

REPO_ROOT = Path(__file__).resolve().parents[2]
LFD_BIN = REPO_ROOT / "target" / "debug" / "lfd"
PG_CONTAINER = "lfd-dev-postgres"
WAVE_NAME = "fork-test"


def ensure_postgres() -> None:
    result = _run_capture(["docker", "inspect", PG_CONTAINER, "--format", "{{.State.Running}}"])
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
        check = _run_capture(
            ["docker", "inspect", PG_CONTAINER, "--format", "{{.State.Health.Status}}"]
        )
        if check.stdout.strip() == "healthy":
            return

    raise RuntimeError("postgres did not become healthy")


def ensure_agent_image() -> None:
    result = _run_capture(["docker", "image", "inspect", "loopflow/agent:latest"])
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
    result = _run_capture(["lsof", "-ti", ":2486"])
    if result.returncode != 0 or not result.stdout.strip():
        return

    for pid in result.stdout.strip().splitlines():
        try:
            os.kill(int(pid), signal.SIGTERM)
        except (ValueError, ProcessLookupError):
            continue
    time.sleep(1)


def start_lfd_container_mode() -> subprocess.Popen[str]:
    kill_existing_lfd()

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

    for _ in range(30):
        time.sleep(0.5)
        check = _run_capture(["curl", "-sf", "http://127.0.0.1:2486/health"])
        if check.returncode == 0:
            print("lfd ready")
            return process

    process.terminate()
    stdout = process.stdout.read() if process.stdout else ""
    raise RuntimeError(f"lfd did not become ready.\nOutput:\n{stdout[:2000]}")


def create_and_run_wave(flow: str, direction: str, wave_name: str = WAVE_NAME) -> None:
    repo = str(REPO_ROOT.resolve())

    try:
        loopflow_api.delete_wave(wave_name)
    except Exception:
        pass

    loopflow_api.create_wave(wave_name, repo=repo, flow=flow, direction=[direction])
    loopflow_api.run_wave(wave_name)
    print(f"Wave '{wave_name}' started with flow '{flow}'")


def wait_for_completion(process: subprocess.Popen[str], timeout: int) -> tuple[bool, str]:
    if process.stdout is None:
        raise RuntimeError("lfd stdout is not available")

    lines: list[str] = []
    deadline = time.time() + timeout
    fork_errors: list[str] = []
    completed_branches = 0
    total_branches = 0

    while time.time() < deadline:
        if process.poll() is not None:
            lines.append(process.stdout.read())
            return False, "".join(lines)

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
            return False, "".join(lines)

        if "synthesize" in stripped and "completed" in stripped.lower():
            return True, "".join(lines)

        if "wave run completed" in stripped or "wave run failed" in stripped:
            return "completed" in stripped, "".join(lines)

    return False, "".join(lines) + "\n[TIMEOUT]"


def _run(cmd: list[str], **kwargs) -> subprocess.CompletedProcess[str]:
    return subprocess.run(cmd, text=True, **kwargs)


def _run_capture(cmd: list[str], **kwargs) -> subprocess.CompletedProcess[str]:
    return subprocess.run(cmd, capture_output=True, text=True, **kwargs)
