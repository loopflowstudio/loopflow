#!/usr/bin/env python3

from __future__ import annotations

import argparse
import subprocess
import sys
from pathlib import Path

CHECKLIST = [
    "1) Open chat and start a new session. Confirm session creation succeeds.",
    "2) Inspect request payload: create-session uses `harness` (not `provider`).",
    "3) Inspect response payload: session includes `harness` and `provider_session_id`.",
    "4) Exercise a bad harness name (e.g. `nonexistent`). Expect HTTP 400.",
    "5) Run two Claude turns and confirm resume continuity (`provider_session_id` persists).",
]


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Launch live review environment for agentapi runtime/naming convergence."
    )
    parser.add_argument(
        "--print-only",
        action="store_true",
        help="Print manual walkthrough checklist without launching Concerto.",
    )
    return parser.parse_args()


def print_checklist() -> None:
    print("\nManual walkthrough checklist\n")
    for item in CHECKLIST:
        print(item)
    print()


def launch_concerto(repo_root: Path) -> int:
    cmd = [
        "uv",
        "run",
        "python",
        "scripts/concerto-dev.py",
        "run-debug",
        "--with-lfd",
    ]
    print(f"Launching: {' '.join(cmd)}\n")
    result = subprocess.run(cmd, cwd=repo_root)
    return result.returncode


def main() -> int:
    args = parse_args()
    repo_root = Path(__file__).resolve().parents[1]

    print_checklist()
    if args.print_only:
        return 0

    return launch_concerto(repo_root)


if __name__ == "__main__":
    sys.exit(main())
