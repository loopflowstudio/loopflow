#!/usr/bin/env python3
"""Validate vendor session launch command and URL shapes."""

import shutil
import subprocess
import sys


def cargo_command() -> list[str]:
    if shutil.which("rustup"):
        result = subprocess.run(
            ["rustup", "toolchain", "list"],
            check=False,
            capture_output=True,
            text=True,
        )
        if result.returncode == 0 and "nightly" in result.stdout:
            return ["cargo", "+nightly"]
    return ["cargo"]


def main() -> int:
    tests = [
        "session_launch_",
        "config_from_yaml_session_launch_",
        "config_session_launch_",
        "default_session_config",
    ]
    cargo = cargo_command()
    for test in tests:
        result = subprocess.run(
            [*cargo, "test", "-p", "loopflow", test],
            check=False,
        )
        if result.returncode != 0:
            return result.returncode
    return 0


if __name__ == "__main__":
    sys.exit(main())
