"""Collector subprocess for capturing agent output."""

import argparse
import itertools
import json
import os
import pty
import signal
import subprocess
import sys
import threading
import time
from pathlib import Path

from loopflow.lf.git import autocommit as git_autocommit
from loopflow.lf.logging import (
    get_model_env,
    open_json_log,
    open_log_file,
    write_log_line,
)
from loopflow.lfd.step_run import _send_fire_and_forget


def _send_output_line(step_run_id: str, text: str) -> None:
    """Send output line to lfd for live streaming. Fire-and-forget."""
    _send_fire_and_forget("output.line", {"step_run_id": step_run_id, "text": text})


def collect_output(
    step_run_id: str,
    command: list[str],
    step: str | None,
    repo_root: Path | None,
    autocommit: bool,
    push: bool,
    interactive: bool,
    foreground: bool,
    prompt: str | None = None,
    token_summary: str | None = None,
    prefix: str | None = None,
) -> int:
    """Run command and collect output to log files."""
    log_file = open_log_file(repo_root, step_run_id)
    json_log = open_json_log(repo_root, step_run_id)

    # Show startup header with token breakdown
    if foreground and token_summary:
        _print_startup_header(step, token_summary)

    if interactive:
        exit_code = _run_interactive(
            command, log_file, json_log, foreground, prompt, step_run_id, prefix
        )
    else:
        exit_code = _run_streaming(
            command, log_file, json_log, foreground, prompt, step_run_id, prefix
        )

    # StepRun status is updated by the parent process via lfd client

    if autocommit and exit_code == 0 and step and repo_root:
        git_autocommit(repo_root, step, push=push)

    if log_file:
        log_file.close()
    if json_log:
        json_log.close()

    return exit_code


def main():
    """Entry point for collector subprocess."""
    parser = argparse.ArgumentParser()
    parser.add_argument("--step-run-id", required=True)
    parser.add_argument("--step", default=None)
    parser.add_argument("--repo-root", default=None)
    parser.add_argument("--autocommit", action="store_true")
    parser.add_argument("--push", action="store_true")
    parser.add_argument("--interactive", action="store_true")
    parser.add_argument("--foreground", action="store_true")
    parser.add_argument(
        "--prompt-file", default=None, help="File containing prompt (or - for stdin)"
    )
    parser.add_argument(
        "--token-summary",
        default=None,
        help="Token breakdown summary to display at startup",
    )
    parser.add_argument(
        "--prefix",
        default=None,
        help="Prefix for output lines (for parallel execution)",
    )
    parser.add_argument("command", nargs=argparse.REMAINDER)

    args = parser.parse_args()

    # Remove -- separator if present
    command = args.command
    if command and command[0] == "--":
        command = command[1:]

    if not command:
        print("Error: No command specified", file=sys.stderr)
        sys.exit(1)

    # Read prompt from file or stdin
    prompt = None
    if args.prompt_file:
        if args.prompt_file == "-":
            prompt = sys.stdin.read()
        else:
            prompt = Path(args.prompt_file).read_text()

    repo_root = Path(args.repo_root).resolve() if args.repo_root else None
    exit_code = collect_output(
        args.step_run_id,
        command,
        args.step,
        repo_root,
        args.autocommit,
        args.push,
        args.interactive,
        args.foreground,
        prompt,
        args.token_summary,
        args.prefix,
    )
    sys.exit(exit_code)


def _print_startup_header(step: str | None, token_summary: str) -> None:
    """Print startup header with token breakdown."""
    step_name = step or "inline"
    print(f"\033[90m━━━ {step_name} ━━━\033[0m", file=sys.stderr)
    # Print token summary in dim
    for line in token_summary.split("\n"):
        print(f"\033[90m{line}\033[0m", file=sys.stderr)
    print(file=sys.stderr)


class _Spinner:
    """Simple spinner for showing activity while waiting."""

    def __init__(self):
        self._frames = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]
        self._cycle = itertools.cycle(self._frames)
        self._stop = threading.Event()
        self._thread = None

    def start(self) -> None:
        """Start the spinner."""
        self._stop.clear()
        self._thread = threading.Thread(target=self._spin, daemon=True)
        self._thread.start()

    def stop(self) -> None:
        """Stop the spinner and clear the line."""
        self._stop.set()
        if self._thread:
            self._thread.join(timeout=0.1)
        # Clear spinner line
        sys.stderr.write("\r\033[K")
        sys.stderr.flush()

    def _spin(self) -> None:
        """Spin until stopped."""
        while not self._stop.is_set():
            frame = next(self._cycle)
            sys.stderr.write(f"\r\033[90m{frame} waiting for response...\033[0m")
            sys.stderr.flush()
            time.sleep(0.1)


def _run_streaming(
    command: list[str],
    log_file,
    json_log,
    foreground: bool,
    prompt: str | None = None,
    step_run_id: str | None = None,
    prefix: str | None = None,
) -> int:
    """Run a non-interactive command and stream output to logs.

    Prompt is passed as a CLI argument.
    """
    use_stdin = (
        prompt is not None and len(command) > 1 and command[0] == "codex" and command[1] == "exec"
    )
    cmd = command + ["-"] if use_stdin else (command + [prompt] if prompt else command)

    process = subprocess.Popen(
        cmd,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        stdin=subprocess.PIPE if use_stdin else None,
        text=True,
        start_new_session=True,
        env=get_model_env(),
    )
    if use_stdin and process.stdin:
        process.stdin.write(prompt)
        process.stdin.close()

    def _handle_signal(signum, frame):
        if process.poll() is None:
            try:
                os.killpg(process.pid, signal.SIGTERM)
            except OSError:
                process.terminate()

    signal.signal(signal.SIGTERM, _handle_signal)

    # Start spinner while waiting for first output
    spinner = _Spinner() if foreground else None
    if spinner:
        spinner.start()
    first_output = True

    for line in process.stdout:
        if not line:
            continue

        # Stop spinner on first output
        if first_output and spinner:
            spinner.stop()
            first_output = False

        raw_line = line.rstrip("\n")

        # Write raw output to JSON log
        if json_log:
            json_log.write(raw_line + "\n")
            json_log.flush()

        # Format for plain text log and console
        formatted_lines = _format_stream_line(raw_line)
        for formatted in formatted_lines:
            # Apply prefix for parallel execution
            display_line = f"{prefix}{formatted}" if prefix else formatted
            write_log_line(log_file, display_line)
            if foreground:
                print(display_line, flush=True)
            if step_run_id:
                _send_output_line(step_run_id, display_line)

    # Stop spinner if no output was received
    if spinner and first_output:
        spinner.stop()

    return process.wait()


def _run_interactive(
    command: list[str],
    log_file,
    json_log,
    foreground: bool,
    prompt: str | None = None,
    step_run_id: str | None = None,
    prefix: str | None = None,
) -> int:
    """Run an interactive command using pty.spawn.

    Prompt is passed as a CLI argument. Prefix is ignored for interactive mode.
    """
    cmd = command + [prompt] if prompt else command

    def master_read(fd: int) -> bytes:
        data = os.read(fd, 4096)
        if data:
            decoded = data.decode(errors="replace")
            if json_log:
                try:
                    json_log.write(decoded)
                    json_log.flush()
                except Exception:
                    pass
            if log_file:
                for line in decoded.splitlines():
                    write_log_line(log_file, line)
                    if step_run_id:
                        _send_output_line(step_run_id, line)
        return data

    # Remove API keys so CLIs use subscriptions instead of API credits
    old_anthropic_key = os.environ.pop("ANTHROPIC_API_KEY", None)
    old_openai_key = os.environ.pop("OPENAI_API_KEY", None)

    try:
        exit_code = pty.spawn(cmd, master_read)
    finally:
        if old_anthropic_key is not None:
            os.environ["ANTHROPIC_API_KEY"] = old_anthropic_key
        if old_openai_key is not None:
            os.environ["OPENAI_API_KEY"] = old_openai_key

    return exit_code


def _format_tool_line(tool: str, input_data: dict | None) -> str:
    """Format a tool invocation line with optional path."""
    path = ""
    if isinstance(input_data, dict) and input_data.get("path"):
        path = f": {input_data['path']}"
    return f"→ {tool}{path}"


def _format_stream_line(line: str) -> list[str]:
    """Format a stream line, parsing JSON if present.

    For read-only auto mode, show only:
    - Tool invocations (what the agent is doing)
    - Agent reasoning (text explanations)
    - Completion status (success/failure)

    Filter out:
    - System initialization messages
    - User messages (prompts, tool results)
    - Internal metadata
    """
    try:
        event = json.loads(line)
    except json.JSONDecodeError:
        return [line]

    event_type = event.get("type")

    # Filter out noise events
    if event_type in ("system", "user"):
        return []

    if event_type == "assistant":
        msg = event.get("message", {})
        content = msg.get("content", [])
        lines = []
        for block in content:
            if block.get("type") == "tool_use":
                lines.append(_format_tool_line(block.get("name", "unknown"), block.get("input")))
            if block.get("type") == "text":
                text = block.get("text", "")
                if text:
                    lines.append(text)
        if lines:
            return lines

    if event_type == "result":
        status = event.get("subtype") or event.get("status") or "unknown"
        return ["✓ Done" if status == "success" else "✗ Failed"]

    # Codex-style events
    if event_type == "tool_use":
        return [_format_tool_line(event.get("tool", "unknown"), event.get("input"))]
    if event_type == "text":
        content = event.get("content", "")
        return [content] if content else []

    # Codex event schema (item.*)
    if event_type in ("item.started", "item.completed"):
        item = event.get("item") or {}
        item_type = item.get("type")
        if item_type == "reasoning":
            text = item.get("text", "")
            return [text] if text else []
        if item_type == "agent_message":
            text = item.get("text", "")
            return [text] if text else []
        if item_type == "command_execution":
            command = item.get("command", "").strip()
            status = item.get("status")
            if event_type == "item.started" and command:
                return [f"→ shell: {command}"]
            if status == "failed" and command:
                return [f"✗ Failed: {command}"]
        return []

    if event_type in ("turn.completed", "thread.completed"):
        return ["✓ Done"]

    return []


if __name__ == "__main__":
    main()
