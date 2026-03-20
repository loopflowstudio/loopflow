#!/usr/bin/env python3
"""Run a live PM priority round-trip check against Linear or Asana."""

import argparse
import subprocess


def run_provider(provider: str) -> int:
    cmd = [
        "cargo",
        "run",
        "--manifest-path",
        "rust/loopflow/Cargo.toml",
        "--example",
        "pm_priority_live",
        "--",
        provider,
    ]
    print(f"\n== {provider} ==")
    result = subprocess.run(cmd, check=False)
    return result.returncode


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--provider",
        choices=["linear", "asana", "both"],
        default="both",
        help="Provider to validate",
    )
    args = parser.parse_args()

    providers = ["linear", "asana"] if args.provider == "both" else [args.provider]
    failures = []
    for provider in providers:
        if run_provider(provider) != 0:
            failures.append(provider)

    if not failures:
        print("\nPASS")
        return 0

    print(f"\nFAIL: {', '.join(failures)}")
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
