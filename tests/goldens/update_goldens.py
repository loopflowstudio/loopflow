from __future__ import annotations

import os
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


def main() -> None:
    """Render goldens with the same isolated, explicitly imported plan as the test."""
    subprocess.run(
        ["cargo", "test", "-p", "loopflow", "--test", "golden_prompt"],
        check=True,
        cwd=ROOT,
        env=dict(os.environ, LOOPFLOW_UPDATE_GOLDENS="1"),
    )


if __name__ == "__main__":
    main()
