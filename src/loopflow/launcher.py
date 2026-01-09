"""Launch LLM coding sessions."""

import json
import subprocess
import sys
from abc import ABC, abstractmethod
from dataclasses import dataclass
from pathlib import Path
from typing import Optional


@dataclass
class LaunchResult:
    """Result from launching a backend."""

    exit_code: int
    output: Optional[str] = None


class Backend(ABC):
    """Abstract base class for LLM backends."""

    @abstractmethod
    def launch(
        self,
        prompt: str,
        print_mode: bool = False,
        stream: bool = False,
        skip_permissions: bool = False,
        cwd: Optional[Path] = None,
    ) -> LaunchResult:
        """Launch a coding session with the given prompt."""
        pass

    @abstractmethod
    def is_available(self) -> bool:
        """Check if the backend CLI is available."""
        pass


class ClaudeBackend(Backend):
    """Claude Code CLI backend."""

    def launch(
        self,
        prompt: str,
        print_mode: bool = False,
        stream: bool = False,
        skip_permissions: bool = False,
        cwd: Optional[Path] = None,
    ) -> LaunchResult:
        """Launch a Claude Code session with the given prompt."""
        exit_code, output = launch_claude(prompt, print_mode, stream, skip_permissions, cwd)
        return LaunchResult(exit_code, output)

    def is_available(self) -> bool:
        """Check if the claude CLI is available."""
        return check_claude_available()


class CodexBackend(Backend):
    """OpenAI Codex CLI backend."""

    def launch(
        self,
        prompt: str,
        print_mode: bool = False,
        stream: bool = False,
        skip_permissions: bool = False,
        cwd: Optional[Path] = None,
    ) -> LaunchResult:
        """Launch a Codex CLI session with the given prompt."""
        cmd = ["codex"]

        if print_mode:
            cmd.append("--quiet")
        if skip_permissions:
            cmd.extend(["--approval-mode", "full-auto"])

        cmd.append(prompt)

        if print_mode:
            result = subprocess.run(cmd, cwd=cwd, capture_output=True, text=True)
            return LaunchResult(result.returncode, result.stdout)
        else:
            result = subprocess.run(cmd, cwd=cwd)
            return LaunchResult(result.returncode, None)

    def is_available(self) -> bool:
        """Check if the codex CLI is available."""
        try:
            subprocess.run(
                ["codex", "--version"],
                capture_output=True,
                check=True,
            )
            return True
        except (subprocess.CalledProcessError, FileNotFoundError):
            return False


def get_backend(name: str) -> Backend:
    """Get a backend instance by name."""
    backends = {
        "claude": ClaudeBackend,
        "codex": CodexBackend,
    }
    if name not in backends:
        raise ValueError(f"Unknown backend: {name}. Available: {list(backends.keys())}")
    return backends[name]()


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
