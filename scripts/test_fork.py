#!/usr/bin/env python3
"""End-to-end test for Docker fork execution.

Builds lfd, starts it with Docker executor, creates a wave with a fork flow,
waits for completion, and reports success/failure. Designed for automated
iteration — exits non-zero on any failure with actionable output.

Usage:
    uv run python scripts/test_fork.py
    uv run python scripts/test_fork.py --flow wave-polish --timeout 300

CI integration:
    This script is the live-fire counterpart to the unit-level Docker tests in
    CI (`cargo test -p loopflow docker_` in ci.yml) and the nightly Docker E2E
    workflow (docker-e2e-nightly.yml). Those test executor mechanics with mocked
    agents; this tests the full path: real lfd → real Docker → real Claude.

    To run in CI, add a job that:
      1. Has ANTHROPIC_API_KEY in secrets
      2. Runs `uv run python scripts/test_fork.py --timeout 600`
      3. Uses `workflow_dispatch` or nightly schedule (not per-PR — costs money)

    See .github/workflows/docker-e2e-nightly.yml for the existing nightly pattern.
"""

import argparse
import os
import re
import signal
import subprocess
import sys
import time
from pathlib import Path

REPO_ROOT = Path(__file__).parent.parent
LFD_BIN = REPO_ROOT / "target" / "debug" / "lfd"
PG_CONTAINER = "lfd-dev-postgres"
WAVE_NAME = "fork-test"


def run(cmd: list[str], **kwargs) -> subprocess.CompletedProcess:
    return subprocess.run(cmd, **kwargs)


def run_capture(cmd: list[str], **kwargs) -> subprocess.CompletedProcess:
    return subprocess.run(cmd, capture_output=True, text=True, **kwargs)


def fail(msg: str) -> None:
    print(f"\nFAIL: {msg}", file=sys.stderr)
    sys.exit(1)


# ---------------------------------------------------------------------------
# Infrastructure setup
# ---------------------------------------------------------------------------
# These helpers mirror scripts/dev.py but are self-contained so the test can
# run standalone in CI without dev.py.


def ensure_postgres() -> None:
    result = run_capture(["docker", "inspect", PG_CONTAINER, "--format", "{{.State.Running}}"])
    if result.returncode == 0 and result.stdout.strip() == "true":
        return

    run(["docker", "rm", "-f", PG_CONTAINER], capture_output=True)
    result = run([
        "docker", "run", "-d",
        "--name", PG_CONTAINER,
        "-p", "5432:5432",
        "-e", "POSTGRES_USER=lfd",
        "-e", "POSTGRES_PASSWORD=lfd",
        "-e", "POSTGRES_DB=lfd",
        "--health-cmd", "pg_isready -U lfd",
        "--health-interval", "2s",
        "--health-retries", "10",
        "postgres:16-alpine",
    ])
    if result.returncode != 0:
        fail("failed to start postgres")

    print("Waiting for postgres...")
    for _ in range(30):
        time.sleep(1)
        check = run_capture([
            "docker", "inspect", PG_CONTAINER,
            "--format", "{{.State.Health.Status}}",
        ])
        if check.stdout.strip() == "healthy":
            return
    fail("postgres did not become healthy")


def ensure_agent_image() -> None:
    result = run_capture(["docker", "image", "inspect", "loopflow/agent:latest"])
    if result.returncode == 0:
        return
    print("Building agent image...")
    result = run(["docker", "build", "-t", "loopflow/agent:latest", "docker/agent"], cwd=REPO_ROOT)
    if result.returncode != 0:
        fail("failed to build agent image")


def build_lfd() -> None:
    print("Building lfd...")
    result = run(["cargo", "build", "--bin", "lfd"], cwd=REPO_ROOT)
    if result.returncode != 0:
        fail("cargo build failed")


# ---------------------------------------------------------------------------
# lfd lifecycle
# ---------------------------------------------------------------------------


def kill_existing_lfd() -> None:
    """Kill any lfd already on port 2486."""
    result = run_capture(["lsof", "-ti", ":2486"])
    if result.returncode == 0 and result.stdout.strip():
        for pid in result.stdout.strip().splitlines():
            try:
                os.kill(int(pid), signal.SIGTERM)
            except (ValueError, ProcessLookupError):
                pass
        time.sleep(1)


def start_lfd() -> subprocess.Popen:
    kill_existing_lfd()

    env = os.environ.copy()
    env["LFD_MODE"] = "container"
    env["LFD_DATABASE_URL"] = "postgres://lfd:lfd@127.0.0.1:5432/lfd"
    env["LFD_EXECUTOR_CREDENTIALS_MOUNTS"] = "claude,ssh,gitconfig"
    env["RUST_LOG"] = "loopflow=debug,tower_http=debug"
    env["GRPC_ENABLE_FORK_SUPPORT"] = "0"
    env["GRPC_VERBOSITY"] = "ERROR"

    # tracing writes to stderr; merge into stdout for unified capture
    proc = subprocess.Popen(
        [str(LFD_BIN), "serve"],
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
        bufsize=1,  # line-buffered
        env=env,
    )

    # Wait for lfd to be ready (listening on port)
    for _ in range(30):
        time.sleep(0.5)
        check = run_capture(["curl", "-sf", "http://127.0.0.1:2486/health"])
        if check.returncode == 0:
            print("lfd ready")
            return proc

    # Dump whatever output we got
    proc.terminate()
    stdout = proc.stdout.read() if proc.stdout else ""
    fail(f"lfd did not become ready.\nOutput:\n{stdout[:2000]}")
    raise SystemExit(1)


# ---------------------------------------------------------------------------
# Wave execution
# ---------------------------------------------------------------------------


def create_and_run_wave(flow: str, direction: str) -> None:
    import loopflow.api as loopflow_api

    repo = str(REPO_ROOT.resolve())

    # Delete existing wave if present
    try:
        loopflow_api.delete_wave(WAVE_NAME)
    except Exception:
        pass

    loopflow_api.create_wave(WAVE_NAME, repo=repo, flow=flow, direction=[direction])
    loopflow_api.run_wave(WAVE_NAME)
    print(f"Wave '{WAVE_NAME}' started with flow '{flow}'")


def wait_for_completion(proc: subprocess.Popen, timeout: int) -> tuple[bool, str]:
    """Read lfd stdout until wave completes or times out.

    Returns (success, collected_output).
    """
    lines: list[str] = []
    deadline = time.time() + timeout
    fork_errors: list[str] = []
    completed_branches = 0
    total_branches = 0

    while time.time() < deadline:
        if proc.poll() is not None:
            remaining = proc.stdout.read() if proc.stdout else ""
            lines.append(remaining)
            return False, "".join(lines)

        line = proc.stdout.readline() if proc.stdout else ""
        if not line:
            time.sleep(0.1)
            continue

        lines.append(line)
        stripped = line.strip()

        # Print key events to stdout for live monitoring
        if any(kw in stripped for kw in [
            "INFO", "WARN", "ERROR",
            "fork branch", "creating agent",
            "running fork", "synthesize",
        ]):
            print(f"  {stripped}")

        # Track fork progress
        if "total_branches=" in stripped:
            m = re.search(r"total_branches=(\d+)", stripped)
            if m:
                total_branches = int(m.group(1))

        if "fork branch error" in stripped:
            fork_errors.append(stripped)

        if "fork branch done" in stripped:
            m = re.search(r"completed=(\d+)", stripped)
            if m:
                completed_branches = int(m.group(1))

        # Early exit: all branches done with errors
        if total_branches > 0 and completed_branches >= total_branches and fork_errors:
            return False, "".join(lines)

        # Success: synthesize completed
        if "synthesize" in stripped and "completed" in stripped.lower():
            return True, "".join(lines)

        # Wave run ended
        if "wave run completed" in stripped or "wave run failed" in stripped:
            success = "completed" in stripped
            return success, "".join(lines)

    return False, "".join(lines) + "\n[TIMEOUT]"


# ---------------------------------------------------------------------------
# Entrypoint
# ---------------------------------------------------------------------------


def main() -> int:
    parser = argparse.ArgumentParser(description="Test Docker fork execution")
    parser.add_argument("--flow", default="wave-reduce")
    parser.add_argument("--direction", default="product-engineer")
    parser.add_argument("--timeout", type=int, default=300, help="Seconds to wait")
    parser.add_argument("--skip-build", action="store_true")
    args = parser.parse_args()

    # Preflight: need either OAuth creds or API key
    claude_dir = Path.home() / ".claude"
    has_oauth = claude_dir.exists() and any(claude_dir.iterdir())
    has_api_key = bool(os.environ.get("ANTHROPIC_API_KEY"))
    if not has_oauth and not has_api_key:
        fail("no Claude credentials found (~/.claude/ or ANTHROPIC_API_KEY)")

    # Setup
    ensure_postgres()
    ensure_agent_image()
    if not args.skip_build:
        build_lfd()

    # Start lfd
    proc = start_lfd()

    try:
        create_and_run_wave(args.flow, args.direction)
        success, output = wait_for_completion(proc, args.timeout)
    finally:
        proc.terminate()
        try:
            proc.wait(timeout=5)
        except subprocess.TimeoutExpired:
            proc.kill()

    if success:
        print("\nPASS: fork execution completed successfully")
        return 0

    # Extract useful failure info
    print("\n--- Failure Analysis ---")
    for line in output.splitlines():
        if any(kw in line for kw in [
            "ERROR", "error=", "WARN",
            "collecting container env", "creating agent",
        ]):
            print(f"  {line.strip()}")

    fail("fork execution failed (see above)")
    return 1


if __name__ == "__main__":
    sys.exit(main())
