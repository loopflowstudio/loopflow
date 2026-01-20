"""Launch LLM coding sessions."""

import json
import subprocess
import sys
from abc import ABC, abstractmethod
from dataclasses import dataclass
from functools import lru_cache
from pathlib import Path
from typing import Optional

from loopflow.lf.logging import (
    get_model_env,
    open_json_log,
    open_log_file,
    write_log_line,
)


@dataclass
class LaunchResult:
    """Result from launching a runner."""

    exit_code: int
    output: Optional[str] = None


class Runner(ABC):
    """Abstract base class for model runners (Claude Code, Codex, etc.)."""

    @abstractmethod
    def launch(
        self,
        prompt: str,
        auto: bool = False,
        stream: bool = False,
        skip_permissions: bool = False,
        model_variant: str | None = None,
        session_id: str | None = None,
        cwd: Optional[Path] = None,
    ) -> LaunchResult:
        """Launch a coding session with the given prompt."""
        pass

    @abstractmethod
    def is_available(self) -> bool:
        """Check if the runner CLI is available."""
        pass


class ClaudeRunner(Runner):
    """Claude Code CLI runner."""

    def launch(
        self,
        prompt: str,
        auto: bool = False,
        stream: bool = False,
        skip_permissions: bool = False,
        model_variant: str | None = None,
        session_id: str | None = None,
        cwd: Optional[Path] = None,
    ) -> LaunchResult:
        exit_code, output = launch_claude(
            prompt, auto, stream, skip_permissions, model_variant, session_id, cwd
        )
        return LaunchResult(exit_code, output)

    def is_available(self) -> bool:
        return check_claude_available()


class CodexRunner(Runner):
    """OpenAI Codex CLI runner."""

    def launch(
        self,
        prompt: str,
        auto: bool = False,
        stream: bool = False,
        skip_permissions: bool = False,
        model_variant: str | None = None,
        session_id: str | None = None,
        cwd: Optional[Path] = None,
    ) -> LaunchResult:
        cmd = build_codex_command(
            auto=auto,
            stream=stream,
            skip_permissions=skip_permissions,
            model_variant=model_variant,
            sandbox_root=cwd.parent if cwd else None,
            workdir=cwd,
        )

        cmd_with_prompt = cmd + [prompt]

        if auto and stream:
            exit_code, output = _run_streaming_json(
                cmd_with_prompt,
                cwd=cwd,
                normalize_fn=normalize_codex_event,
                session_id=session_id,
            )
            return LaunchResult(exit_code, output)
        if auto:
            result = subprocess.run(
                cmd_with_prompt,
                cwd=cwd,
                capture_output=True,
                text=True,
                env=get_model_env(),
            )
            return LaunchResult(result.returncode, result.stdout)

        result = subprocess.run(cmd_with_prompt, cwd=cwd, text=True, env=get_model_env())
        return LaunchResult(result.returncode, None)

    def is_available(self) -> bool:
        return check_codex_available()


class GeminiRunner(Runner):
    """Google Gemini CLI runner."""

    def launch(
        self,
        prompt: str,
        auto: bool = False,
        stream: bool = False,
        skip_permissions: bool = False,
        model_variant: str | None = None,
        session_id: str | None = None,
        cwd: Optional[Path] = None,
    ) -> LaunchResult:
        cmd = build_gemini_command(
            auto=auto,
            stream=stream,
            skip_permissions=skip_permissions,
            model_variant=model_variant,
            sandbox_root=cwd.parent if cwd else None,
            workdir=cwd,
        )

        cmd_with_prompt = cmd + [prompt]

        if auto and stream:
            exit_code, output = _run_streaming_json(
                cmd_with_prompt,
                cwd=cwd,
                normalize_fn=normalize_gemini_event,
                session_id=session_id,
            )
            return LaunchResult(exit_code, output)
        if auto:
            result = subprocess.run(
                cmd_with_prompt,
                cwd=cwd,
                capture_output=True,
                text=True,
                env=get_model_env(),
            )
            return LaunchResult(result.returncode, result.stdout)

        result = subprocess.run(cmd_with_prompt, cwd=cwd, text=True, env=get_model_env())
        return LaunchResult(result.returncode, None)

    def is_available(self) -> bool:
        return check_gemini_available()


def get_runner(model: str) -> Runner:
    """Get a runner instance for the given model."""
    runners = {
        "claude": ClaudeRunner,
        "codex": CodexRunner,
        "gemini": GeminiRunner,
    }
    if model not in runners:
        raise ValueError(f"Unknown model: {model}. Available: {list(runners.keys())}")
    return runners[model]()


def launch_claude(
    prompt: str,
    auto: bool = False,
    stream: bool = False,
    skip_permissions: bool = False,
    model_variant: str | None = None,
    session_id: str | None = None,
    cwd: Optional[Path] = None,
) -> tuple[int, str | None]:
    """Launch a Claude Code session with the given prompt.

    Returns (exit_code, output). Output is only captured in print mode.
    Prompt is passed as a CLI argument.
    """
    cmd = build_claude_command(
        auto=auto,
        stream=stream,
        skip_permissions=skip_permissions,
        model_variant=model_variant,
    )
    cmd_with_prompt = cmd + [prompt]

    if auto and stream:
        return _run_streaming_json(
            cmd_with_prompt,
            cwd=cwd,
            normalize_fn=normalize_claude_event,
            session_id=session_id,
        )
    elif auto:
        result = subprocess.run(
            cmd_with_prompt,
            cwd=cwd,
            capture_output=True,
            text=True,
            env=get_model_env(),
        )
        return result.returncode, result.stdout
    else:
        result = subprocess.run(cmd_with_prompt, cwd=cwd, text=True, env=get_model_env())
        return result.returncode, None


def build_claude_command(
    auto: bool,
    stream: bool,
    skip_permissions: bool,
    model_variant: str | None = None,
    chrome: bool = False,
) -> list[str]:
    """Build Claude CLI command for the requested run mode.

    Prompt should be appended as a CLI argument.
    """
    cmd = ["claude"]

    if chrome:
        cmd.append("--chrome")

    if model_variant:
        cmd.extend(["--model", model_variant])

    if auto:
        # Batch mode always skips permissions (no way to grant them interactively)
        cmd.extend(["--print", "--dangerously-skip-permissions"])
        if stream:
            cmd.extend(["--output-format", "stream-json", "--verbose"])
    elif skip_permissions:
        cmd.append("--dangerously-skip-permissions")

    return cmd


def build_codex_command(
    auto: bool,
    stream: bool,
    skip_permissions: bool,
    yolo: bool = False,
    model_variant: str | None = None,
    sandbox_root: Path | None = None,
    workdir: Path | None = None,
    images: list[Path] | None = None,
) -> list[str]:
    """Build Codex CLI command for the requested run mode.

    Prompt should be appended as a CLI argument.
    yolo bypasses approvals and sandboxing.
    """
    cmd = ["codex", "exec"]

    if model_variant:
        cmd.extend(["-c", f'model="{model_variant}"'])

    if workdir:
        cmd.extend(["-C", str(workdir)])

    if stream:
        cmd.append("--json")

    if yolo:
        cmd.append("--dangerously-bypass-approvals-and-sandbox")
    else:
        cmd.extend(["--sandbox", "workspace-write"])
        if sandbox_root:
            cmd.extend(["--add-dir", str(sandbox_root)])

    # Attach images via -i flag
    if images:
        for img in images:
            cmd.extend(["-i", str(img)])

    if not yolo:
        if skip_permissions:
            # Keep sandboxing but avoid approval prompts in exec mode.
            cmd.extend(["-c", 'approval_policy="never"'])
        elif auto:
            if _codex_exec_supports_full_auto():
                cmd.append("--full-auto")
            else:
                cmd.extend(["-c", 'approval_policy="on-request"'])

    return cmd


@lru_cache(maxsize=1)
def _codex_exec_supports_full_auto() -> bool:
    """Check whether codex exec supports --full-auto."""
    try:
        result = subprocess.run(
            ["codex", "exec", "--help"],
            capture_output=True,
            text=True,
            check=True,
        )
    except (subprocess.CalledProcessError, FileNotFoundError):
        return False
    return "--full-auto" in result.stdout


def build_codex_interactive_command(
    skip_permissions: bool,
    yolo: bool = False,
    model_variant: str | None = None,
    sandbox_root: Path | None = None,
    workdir: Path | None = None,
    images: list[Path] | None = None,
) -> list[str]:
    """Build Codex CLI command for interactive mode.

    Prompt should be appended as a CLI argument.
    yolo bypasses approvals and sandboxing.
    """
    cmd = ["codex"]
    if model_variant:
        cmd.extend(["-c", f'model="{model_variant}"'])
    if workdir:
        cmd.extend(["-C", str(workdir)])
    if yolo:
        cmd.append("--dangerously-bypass-approvals-and-sandbox")
    else:
        if skip_permissions:
            cmd.extend(["-a", "never"])
        cmd.extend(["--sandbox", "workspace-write"])
        if sandbox_root:
            cmd.extend(["--add-dir", str(sandbox_root)])
    # Attach images via -i flag
    if images:
        for img in images:
            cmd.extend(["-i", str(img)])
    return cmd


def build_gemini_command(
    auto: bool,
    stream: bool,
    skip_permissions: bool,
    model_variant: str | None = None,
    sandbox_root: Path | None = None,
    workdir: Path | None = None,
) -> list[str]:
    """Build Gemini CLI command.

    auto mode: uses positional prompt (no -p flag needed)
    stream mode: --output-format stream-json
    skip_permissions: --yolo
    model_variant: -m <variant>
    sandbox_root: included via --include-directories for git access
    workdir: not used as CLI arg (handled via cwd in subprocess)
    """
    cmd = ["gemini"]

    if model_variant:
        cmd.extend(["-m", model_variant])

    if stream:
        cmd.extend(["--output-format", "stream-json"])

    if skip_permissions:
        cmd.append("--yolo")

    if sandbox_root:
        cmd.extend(["--include-directories", str(sandbox_root)])

    return cmd


def build_gemini_interactive_command(
    skip_permissions: bool,
    model_variant: str | None = None,
    sandbox_root: Path | None = None,
    workdir: Path | None = None,
) -> list[str]:
    """Build Gemini CLI command for interactive mode.

    Uses -i to accept prompt then continue interactively.
    """
    cmd = ["gemini"]

    if model_variant:
        cmd.extend(["-m", model_variant])

    if skip_permissions:
        cmd.append("--yolo")

    if sandbox_root:
        cmd.extend(["--include-directories", str(sandbox_root)])

    cmd.append("-i")

    return cmd


def build_model_command(
    model: str,
    auto: bool,
    stream: bool,
    skip_permissions: bool,
    yolo: bool = False,
    model_variant: str | None = None,
    sandbox_root: Path | None = None,
    workdir: Path | None = None,
    images: list[Path] | None = None,
    chrome: bool = False,
) -> list[str]:
    """Build a model command for auto/background execution.

    Prompt should be appended as a CLI argument.
    Images are passed to Codex via -i flag; Claude/Gemini read from filesystem.
    """
    if model == "claude":
        return build_claude_command(
            auto=auto,
            stream=stream,
            skip_permissions=skip_permissions,
            model_variant=model_variant,
            chrome=chrome,
        )
    if model == "gemini":
        return build_gemini_command(
            auto=auto,
            stream=stream,
            skip_permissions=skip_permissions,
            model_variant=model_variant,
            sandbox_root=sandbox_root,
            workdir=workdir,
        )
    return build_codex_command(
        auto=auto,
        stream=stream,
        skip_permissions=skip_permissions,
        yolo=yolo,
        model_variant=model_variant,
        sandbox_root=sandbox_root,
        workdir=workdir,
        images=images,
    )


def build_model_interactive_command(
    model: str,
    skip_permissions: bool,
    yolo: bool = False,
    model_variant: str | None = None,
    sandbox_root: Path | None = None,
    workdir: Path | None = None,
    images: list[Path] | None = None,
    chrome: bool = False,
) -> list[str]:
    """Build a model command for interactive execution.

    Prompt should be appended as a CLI argument.
    Images are passed to Codex via -i flag; Claude/Gemini read from filesystem.
    """
    if model == "claude":
        return build_claude_command(
            auto=False,
            stream=False,
            skip_permissions=skip_permissions,
            model_variant=model_variant,
            chrome=chrome,
        )
    if model == "gemini":
        return build_gemini_interactive_command(
            skip_permissions=skip_permissions,
            model_variant=model_variant,
            sandbox_root=sandbox_root,
            workdir=workdir,
        )
    return build_codex_interactive_command(
        skip_permissions=skip_permissions,
        yolo=yolo,
        model_variant=model_variant,
        sandbox_root=sandbox_root,
        workdir=workdir,
        images=images,
    )


def _run_streaming_json(
    cmd: list[str],
    cwd: Optional[Path],
    normalize_fn,
    session_id: str | None,
) -> tuple[int, str | None]:
    """Run a CLI with JSON streaming output and emit normalized events.

    Prompt should be included in cmd as a CLI argument.
    """
    process = subprocess.Popen(
        cmd,
        cwd=cwd,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
        env=get_model_env(),
    )

    result_text = None
    log_file = open_log_file(cwd, session_id)
    json_log = open_json_log(cwd, session_id)

    for line in process.stdout:
        line = line.strip()
        if not line:
            continue

        # Write raw output to JSON log
        if json_log:
            json_log.write(line + "\n")
            json_log.flush()

        try:
            event = json.loads(line)
        except json.JSONDecodeError:
            event = {"type": "text", "content": line}

        normalized_events = normalize_fn(event)
        for normalized in normalized_events:
            formatted = _format_normalized_event(normalized)
            if formatted:
                _print_status(formatted) if formatted.startswith("→") else print(
                    formatted, end="", flush=True
                )
                write_log_line(log_file, formatted)

            if normalized.get("type") == "result":
                result_text = normalized.get("status")

    process.wait()
    if log_file:
        log_file.close()
    if json_log:
        json_log.close()
    return process.returncode, result_text


def normalize_claude_event(event: dict) -> list[dict]:
    """Normalize Claude stream-json events to common schema."""
    event_type = event.get("type")
    results = []

    if event_type == "assistant":
        msg = event.get("message", {})
        content = msg.get("content", [])
        for block in content:
            if block.get("type") == "tool_use":
                results.append(
                    {
                        "type": "tool_use",
                        "tool": block.get("name", "unknown"),
                        "input": block.get("input", {}),
                    }
                )
            elif block.get("type") == "text":
                text = block.get("text", "")
                if text:
                    results.append(
                        {
                            "type": "text",
                            "content": text,
                        }
                    )

    elif event_type == "result":
        results.append(
            {
                "type": "result",
                "status": event.get("subtype", "unknown"),
            }
        )

    return results


def normalize_codex_event(event: dict) -> list[dict]:
    """Normalize Codex JSON events to common schema."""
    return [event] if event else []


def normalize_gemini_event(event: dict) -> list[dict]:
    """Normalize Gemini stream-json events to common schema."""
    event_type = event.get("type")
    results = []

    if event_type == "text":
        text = event.get("content", "")
        if text:
            results.append({"type": "text", "content": text})

    elif event_type == "tool_use":
        results.append(
            {
                "type": "tool_use",
                "tool": event.get("name", "unknown"),
                "input": event.get("input", {}),
            }
        )

    elif event_type == "result":
        results.append(
            {
                "type": "result",
                "status": event.get("status", "unknown"),
            }
        )

    return results


def _format_normalized_event(event: dict) -> str | None:
    """Format a normalized event in unified format."""
    event_type = event.get("type")
    if event_type == "tool_use":
        tool = event.get("tool", "unknown")
        path = ""
        input_data = event.get("input") or {}
        if isinstance(input_data, dict) and input_data.get("path"):
            path = f": {input_data['path']}"
        return f"→ {tool}{path}"
    elif event_type == "text":
        text = event.get("content", "")
        return text if text else None
    elif event_type == "result":
        status = event.get("status", "unknown")
        if status == "success":
            return "\n✓ Done\n"
        elif status == "error":
            return "✗ Failed\n"
    return None


def _print_status(msg: str) -> None:
    """Print a status message."""
    print(f"\033[90m{msg}\033[0m", file=sys.stderr)


def _check_cli_available(cli_name: str) -> bool:
    """Check if a CLI tool is available by running --version."""
    try:
        subprocess.run(
            [cli_name, "--version"],
            capture_output=True,
            check=True,
        )
        return True
    except (subprocess.CalledProcessError, FileNotFoundError):
        return False


def check_claude_available() -> bool:
    """Check if the claude CLI is available."""
    return _check_cli_available("claude")


def check_codex_available() -> bool:
    """Check if the codex CLI is available."""
    return _check_cli_available("codex")


def check_gemini_available() -> bool:
    """Check if the gemini CLI is available."""
    return _check_cli_available("gemini")
