#!/usr/bin/env python3
"""End-to-end test for Docker fork execution."""

from __future__ import annotations

import argparse
import os
import subprocess
import sys
from pathlib import Path

from lib.fork_scenarios import (
    build_lfd,
    create_and_run_wave,
    ensure_agent_image,
    ensure_postgres,
    start_lfd_container_mode,
    wait_for_completion,
)


def main() -> int:
    parser = argparse.ArgumentParser(description="Test Docker fork execution")
    parser.add_argument("--flow", default="wave-reduce")
    parser.add_argument("--direction", default="product-engineer")
    parser.add_argument("--timeout", type=int, default=300, help="Seconds to wait")
    parser.add_argument("--skip-build", action="store_true")
    args = parser.parse_args()

    if not _has_claude_credentials():
        return _fail("no Claude credentials found (~/.claude/ or ANTHROPIC_API_KEY)")

    try:
        ensure_postgres()
        ensure_agent_image()
        if not args.skip_build:
            build_lfd()
    except RuntimeError as exc:
        return _fail(str(exc))

    try:
        process = start_lfd_container_mode()
    except RuntimeError as exc:
        return _fail(str(exc))
    try:
        create_and_run_wave(args.flow, args.direction)
        success, output = wait_for_completion(process, args.timeout)
    except RuntimeError as exc:
        output = str(exc)
        success = False
    finally:
        process.terminate()
        try:
            process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            process.kill()

    if success:
        print("\nPASS: fork execution completed successfully")
        return 0

    print("\n--- Failure Analysis ---")
    for line in output.splitlines():
        if any(
            keyword in line
            for keyword in ["ERROR", "error=", "WARN", "collecting container env", "creating agent"]
        ):
            print(f"  {line.strip()}")

    return _fail("fork execution failed (see above)")


def _has_claude_credentials() -> bool:
    claude_dir = Path.home() / ".claude"
    has_oauth = claude_dir.exists() and any(claude_dir.iterdir())
    has_api_key = bool(os.environ.get("ANTHROPIC_API_KEY"))
    return has_oauth or has_api_key


def _fail(message: str) -> int:
    print(f"\nFAIL: {message}", file=sys.stderr)
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
