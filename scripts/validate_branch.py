#!/usr/bin/env python3
"""Validate the current branch, then launch Concerto for manual UI walkthrough.

Runs automated checks (fmt, clippy, cargo test, swift test, python tests),
then starts run-debug (lfd + Concerto) for interactive verification.

Usage:
    uv run python scripts/validate_branch.py
    uv run python scripts/validate_branch.py --checks-only   # no UI launch
"""

import argparse
import subprocess
import sys
import time
from pathlib import Path

REPO_ROOT = Path(__file__).parent.parent
SWIFT_DIR = REPO_ROOT / "swift"


def run(cmd: list[str], label: str, cwd: Path = REPO_ROOT) -> bool:
    print(f"\n{'=' * 60}")
    print(f"  {label}")
    print(f"{'=' * 60}\n")
    start = time.monotonic()
    result = subprocess.run(cmd, cwd=cwd)
    elapsed = time.monotonic() - start
    ok = result.returncode == 0
    status = "PASS" if ok else "FAIL"
    print(f"\n  [{status}] {label} ({elapsed:.1f}s)")
    return ok


def main() -> int:
    parser = argparse.ArgumentParser(description="Validate branch and launch UI")
    parser.add_argument("--checks-only", action="store_true", help="Run checks only, don't launch UI")
    args = parser.parse_args()

    checks: list[tuple[list[str], str]] = [
        (["cargo", "fmt", "--all", "--", "--check"], "cargo fmt"),
        (["cargo", "clippy", "--all-targets", "--", "-D", "warnings"], "cargo clippy"),
        (["cargo", "test", "--all"], "cargo test"),
        (["uv", "run", "pytest", "python/tests/"], "python tests"),
        (["swift", "test", "--package-path", str(SWIFT_DIR)], "swift tests"),
        (
            ["uv", "run", "python", "scripts/check_swift_multiplatform_boundaries.py"],
            "swift multiplatform boundary checks",
        ),
    ]

    results: list[tuple[str, bool]] = []
    for cmd, label in checks:
        ok = run(cmd, label)
        results.append((label, ok))

    print(f"\n{'=' * 60}")
    print("  Results")
    print(f"{'=' * 60}")
    all_pass = True
    for label, ok in results:
        status = "PASS" if ok else "FAIL"
        print(f"  [{status}] {label}")
        if not ok:
            all_pass = False

    if not all_pass:
        print("\nSome checks failed. Fix before manual walkthrough.")
        return 1

    print("\nAll checks passed.")

    if args.checks_only:
        return 0

    print(f"\n{'=' * 60}")
    print("  Launching Concerto (run-debug)")
    print(f"{'=' * 60}")
    print()
    print("Manual walkthrough:")
    print("  1. Select a wave with a README containing ## Vision / ## Goals / ## Risks")
    print("  2. Wave detail → Current tab: goals, risks, roadmap should render")
    print("  3. Wave detail → Runs tab: run history appears and combine flow is available")
    print('  4. Sidebar → "Start a wave": enter a wave name and pick a step')
    print("  5. Start wave → interactive session launches in embedded terminal")
    print()

    return subprocess.run(
        ["uv", "run", "python", "scripts/dev.py", "run-debug"],
        cwd=REPO_ROOT,
    ).returncode


if __name__ == "__main__":
    sys.exit(main())
