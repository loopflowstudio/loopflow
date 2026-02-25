#!/usr/bin/env python3
"""End-to-end test for Docker fork execution."""

from __future__ import annotations

import argparse
import sys

from lib.fork_scenarios import (
    build_lfd,
    cleanup_wave_artifacts,
    create_test_wave_name,
    create_and_run_wave,
    ensure_agent_image,
    ensure_postgres,
    has_claude_credentials,
    start_lfd_container_mode,
    stop_process,
    wait_for_completion,
)


def main() -> int:
    parser = argparse.ArgumentParser(description="Test Docker fork execution")
    parser.add_argument("--flow", default="wave-reduce")
    parser.add_argument("--direction", default="ux")
    parser.add_argument("--timeout", type=int, default=600, help="Seconds to wait")
    parser.add_argument("--skip-build", action="store_true")
    args = parser.parse_args()

    if not has_claude_credentials():
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
    wave_name = create_test_wave_name()
    try:
        create_and_run_wave(args.flow, args.direction, wave_name=wave_name)
        success, output = wait_for_completion(process, args.timeout, wave_name=wave_name)
    except RuntimeError as exc:
        output = str(exc)
        success = False
    finally:
        cleanup_wave_artifacts(wave_name)
        stop_process(process)

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


def _fail(message: str) -> int:
    print(f"\nFAIL: {message}", file=sys.stderr)
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
