"""Demonstrate stale-base PR opening with disposable Git and simulated GitHub."""

import os
import subprocess
from pathlib import Path


def main() -> None:
    repo = Path(__file__).resolve().parent.parent
    environment = {key: value for key, value in os.environ.items() if not key.startswith("LF_")}
    if os.uname().sysname == "Darwin":
        environment["TMPDIR"] = "/private/tmp"
    with open(os.devnull) as stdin:
        subprocess.run(
            [
                "uv",
                "run",
                "python",
                "scripts/test_network.py",
                "cargo",
                "test",
                "-p",
                "loopflow",
                "--test",
                "task_pr_range_tests",
                "pr_open_resolves_stale_bases_without_rewriting_history",
                "--",
                "--nocapture",
            ],
            cwd=repo,
            env=environment,
            stdin=stdin,
            check=True,
        )


if __name__ == "__main__":
    main()
