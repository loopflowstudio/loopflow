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


def get_model_env(strip_api_keys: bool = True) -> dict[str, str]:
    """Get environment for model subprocess.

    Removes API keys so CLIs use subscriptions instead of API credits:
    - ANTHROPIC_API_KEY removed so Claude uses Max subscription
    - OPENAI_API_KEY removed so Codex uses ChatGPT subscription
    """
    env = os.environ.copy()
    if strip_api_keys:
        env.pop("ANTHROPIC_API_KEY", None)
        env.pop("OPENAI_API_KEY", None)
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
