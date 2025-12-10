"""Launch LLM coding sessions."""

import subprocess
from pathlib import Path


def launch_claude(
    prompt: str,
    print_mode: bool = False,
    cwd: Path | None = None,
) -> tuple[int, str | None]:
    """Launch a Claude Code session with the given prompt.

    Returns (exit_code, output). Output is only captured in print mode.
    """
    cmd = ["claude"]

    if print_mode:
        cmd.append("--print")

    cmd.append(prompt)

    if print_mode:
        result = subprocess.run(cmd, cwd=cwd, capture_output=True, text=True)
        return result.returncode, result.stdout
    else:
        result = subprocess.run(cmd, cwd=cwd)
        return result.returncode, None


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
