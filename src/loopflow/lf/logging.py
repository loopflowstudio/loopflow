"""Shared logging utilities for session output."""

import os
import tempfile
from datetime import datetime
from pathlib import Path
from typing import Optional, TextIO


def write_prompt_file(prompt: str) -> str:
    """Write prompt to a temp file and return the path.

    The temp file is not deleted automatically; the caller should clean it up.
    """
    fd, path = tempfile.mkstemp(prefix="lf-prompt-", suffix=".txt")
    os.write(fd, prompt.encode())
    os.close(fd)
    return path


def write_prompt_log(
    repo_root: Path,
    prompt: str,
    step_name: str,
    flow_parents: list[str] | None = None,
) -> Path:
    """Write prompt to .lf/log/ and return the path.

    File format: {timestamp}-{flow_parents}.{step}.md or {timestamp}-{step}.md
    Files are preserved for debugging/history.

    Ensures .lf/log/ is in the repo's root .gitignore.
    """
    log_dir = repo_root / ".lf" / "log"
    log_dir.mkdir(parents=True, exist_ok=True)

    # Ensure .lf/log/ is in repo .gitignore
    _ensure_gitignore_entry(repo_root, ".lf/log/")

    timestamp = datetime.now().strftime("%Y%m%d-%H%M%S")
    if flow_parents:
        name_part = f"{'.'.join(flow_parents)}.{step_name}"
    else:
        name_part = step_name
    filename = f"{timestamp}-{name_part}.md"
    path = log_dir / filename

    path.write_text(prompt, encoding="utf-8")
    return path


def _ensure_gitignore_entry(repo_root: Path, entry: str) -> None:
    """Ensure an entry exists in the repo's root .gitignore."""
    gitignore_path = repo_root / ".gitignore"
    entry_line = entry.strip()

    if gitignore_path.exists():
        content = gitignore_path.read_text(encoding="utf-8")
        if any(line.strip() == entry_line for line in content.splitlines()):
            return
        # Add entry on new line
        if content and not content.endswith("\n"):
            content += "\n"
        content += entry_line + "\n"
        gitignore_path.write_text(content, encoding="utf-8")
    else:
        gitignore_path.write_text(entry_line + "\n", encoding="utf-8")


def get_model_env(
    strip_api_keys: bool = True,
    gemini_context_file: Path | None = None,
) -> dict[str, str]:
    """Get environment for model subprocess.

    Removes API keys so CLIs use subscriptions instead of API credits:
    - ANTHROPIC_API_KEY removed so Claude uses Max subscription
    - OPENAI_API_KEY removed so Codex uses ChatGPT subscription

    For Gemini, sets GEMINI_SYSTEM_MD to load context from file.
    """
    env = os.environ.copy()
    if strip_api_keys:
        env.pop("ANTHROPIC_API_KEY", None)
        env.pop("OPENAI_API_KEY", None)
    if gemini_context_file:
        env["GEMINI_SYSTEM_MD"] = str(gemini_context_file)
    return env


def get_log_dir(repo_root: Optional[Path]) -> Path:
    """Get log directory for a session.

    Logs are stored at ~/.lf/logs/{worktree}/.
    """
    worktree = repo_root.name if repo_root else "unknown"
    log_dir = Path.home() / ".lf" / "logs" / worktree
    log_dir.mkdir(parents=True, exist_ok=True)
    return log_dir


def open_log_file(repo_root: Optional[Path], session_id: str) -> Optional[TextIO]:
    """Open plain text log file for this session."""
    if not session_id:
        return None
    log_dir = get_log_dir(repo_root)
    log_path = log_dir / f"{session_id}.log"
    return log_path.open("a", encoding="utf-8")


def open_json_log(repo_root: Optional[Path], session_id: str) -> Optional[TextIO]:
    """Open JSON log file for raw model output."""
    if not session_id:
        return None
    log_dir = get_log_dir(repo_root)
    log_path = log_dir / f"{session_id}.jsonl"
    return log_path.open("a", encoding="utf-8")


def write_log_line(log_file: Optional[TextIO], line: str) -> None:
    """Write a line to the log file with a timestamp."""
    if not log_file:
        return
    timestamp = datetime.now().isoformat()
    log_file.write(f"[{timestamp}] {line}")
    if not line.endswith("\n"):
        log_file.write("\n")
    log_file.flush()
