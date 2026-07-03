#!/usr/bin/env python3
"""Run a live PM priority round-trip check against Asana."""

import subprocess


def main() -> int:
    cmd = [
        "cargo",
        "run",
        "--manifest-path",
        "rust/loopflow/Cargo.toml",
        "--example",
        "pm_priority_live",
        "--",
        "asana",
    ]
    print("\n== asana ==")
    if subprocess.run(cmd, check=False).returncode != 0:
        print("\nFAIL: asana")
        return 1

    print("\nPASS")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
