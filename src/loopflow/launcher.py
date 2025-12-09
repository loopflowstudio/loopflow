"""Launch LLM coding sessions."""

import subprocess
import sys
from pathlib import Path


def launch_claude(
    prompt: str,
    print_mode: bool = False,
    cwd: Path | None = None,
) -> int:
    """Launch a Claude Code session with the given prompt."""
    cmd = ["claude"]

    if print_mode:
        cmd.append("--print")

    cmd.extend(["--prompt", prompt])

    result = subprocess.run(cmd, cwd=cwd)
    return result.returncode


def check_claude_available() -> bool:
    """Check if the claude CLI is available."""
    try:
        subprocess.run(
            ["claude", "--version"],
            capture_output=True,
            check=True,
        )
        return True
    except (subprocess.CalledProcessError, FileNotFoundError):
        return False
