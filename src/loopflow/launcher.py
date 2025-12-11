"""Launch LLM coding sessions."""

import json
import subprocess
import sys
from pathlib import Path
from typing import Optional


def launch_claude(
    prompt: str,
    print_mode: bool = False,
    stream: bool = False,
    skip_permissions: bool = False,
    cwd: Optional[Path] = None,
) -> tuple[int, str | None]:
    """Launch a Claude Code session with the given prompt.

    Returns (exit_code, output). Output is only captured in print mode.
    """
    cmd = ["claude"]

    if print_mode:
        # Batch mode always skips permissions (no way to grant them interactively)
        cmd.extend(["--print", "--dangerously-skip-permissions"])
        if stream:
            cmd.extend(["--output-format", "stream-json", "--verbose"])
    elif skip_permissions:
        cmd.append("--dangerously-skip-permissions")

    cmd.append(prompt)

    if print_mode and stream:
        return _run_streaming(cmd, cwd)
    elif print_mode:
        result = subprocess.run(cmd, cwd=cwd, capture_output=True, text=True)
        return result.returncode, result.stdout
    else:
        result = subprocess.run(cmd, cwd=cwd)
        return result.returncode, None


def _run_streaming(cmd: list[str], cwd: Optional[Path]) -> tuple[int, str | None]:
    """Run claude with streaming output, displaying progress."""
    process = subprocess.Popen(
        cmd,
        cwd=cwd,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    result_text = None

    for line in process.stdout:
        line = line.strip()
        if not line:
            continue

        try:
            event = json.loads(line)
        except json.JSONDecodeError:
            continue

        _handle_stream_event(event)

        # Capture final result
        if event.get("type") == "result":
            result_text = event.get("result")

    process.wait()
    return process.returncode, result_text


def _handle_stream_event(event: dict) -> None:
    """Print relevant streaming events."""
    event_type = event.get("type")
    subtype = event.get("subtype")

    if event_type == "assistant":
        msg = event.get("message", {})
        content = msg.get("content", [])
        for block in content:
            if block.get("type") == "tool_use":
                tool = block.get("name", "unknown")
                _print_status(f"→ {tool}")
            elif block.get("type") == "text":
                text = block.get("text", "")
                if text:
                    # Print assistant text output
                    print(text, end="", flush=True)

    elif event_type == "result":
        if subtype == "success":
            print()  # Newline after streaming text
        elif subtype == "error":
            error = event.get("error", "Unknown error")
            _print_status(f"✗ {error}")


def _print_status(msg: str) -> None:
    """Print a status message."""
    print(f"\033[90m{msg}\033[0m", file=sys.stderr)


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
