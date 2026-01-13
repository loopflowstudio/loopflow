"""Tests for logging utilities."""

import os
from datetime import datetime
from io import StringIO
from pathlib import Path

from loopflow.logging import (
    get_log_dir,
    get_model_env,
    open_json_log,
    open_log_file,
    write_log_line,
    write_prompt_file,
)


def test_get_model_env_removes_anthropic_key(monkeypatch):
    """get_model_env removes ANTHROPIC_API_KEY."""
    monkeypatch.setenv("ANTHROPIC_API_KEY", "secret-key")
    monkeypatch.setenv("OTHER_VAR", "keep-me")

    env = get_model_env()

    assert "ANTHROPIC_API_KEY" not in env
    assert env.get("OTHER_VAR") == "keep-me"


def test_get_model_env_removes_openai_key(monkeypatch):
    """get_model_env removes OPENAI_API_KEY so Codex uses subscription."""
    monkeypatch.setenv("OPENAI_API_KEY", "openai-key")

    env = get_model_env()

    assert "OPENAI_API_KEY" not in env


def test_get_model_env_handles_missing_key(monkeypatch):
    """get_model_env works when ANTHROPIC_API_KEY is not set."""
    monkeypatch.delenv("ANTHROPIC_API_KEY", raising=False)

    env = get_model_env()

    assert "ANTHROPIC_API_KEY" not in env


def test_get_log_dir_creates_directory(tmp_path, monkeypatch):
    """get_log_dir creates log directory structure."""
    monkeypatch.setattr(Path, "home", lambda: tmp_path)

    log_dir = get_log_dir(Path("/project/my-worktree"))

    assert log_dir == tmp_path / ".lf" / "logs" / "my-worktree"
    assert log_dir.exists()


def test_get_log_dir_uses_unknown_for_none(tmp_path, monkeypatch):
    """get_log_dir uses 'unknown' when repo_root is None."""
    monkeypatch.setattr(Path, "home", lambda: tmp_path)

    log_dir = get_log_dir(None)

    assert log_dir == tmp_path / ".lf" / "logs" / "unknown"
    assert log_dir.exists()


def test_open_log_file_creates_file(tmp_path, monkeypatch):
    """open_log_file creates a .log file."""
    monkeypatch.setattr(Path, "home", lambda: tmp_path)

    handle = open_log_file(Path("/project/wt"), "session-123")

    assert handle is not None
    handle.write("test\n")
    handle.close()

    log_path = tmp_path / ".lf" / "logs" / "wt" / "session-123.log"
    assert log_path.exists()
    assert log_path.read_text() == "test\n"


def test_open_log_file_returns_none_for_empty_session():
    """open_log_file returns None when session_id is empty."""
    handle = open_log_file(Path("/project"), "")

    assert handle is None


def test_open_json_log_creates_jsonl_file(tmp_path, monkeypatch):
    """open_json_log creates a .jsonl file."""
    monkeypatch.setattr(Path, "home", lambda: tmp_path)

    handle = open_json_log(Path("/project/wt"), "session-456")

    assert handle is not None
    handle.write('{"type": "test"}\n')
    handle.close()

    log_path = tmp_path / ".lf" / "logs" / "wt" / "session-456.jsonl"
    assert log_path.exists()
    assert log_path.read_text() == '{"type": "test"}\n'


def test_open_json_log_returns_none_for_empty_session():
    """open_json_log returns None when session_id is empty."""
    handle = open_json_log(Path("/project"), "")

    assert handle is None


def test_write_log_line_adds_timestamp():
    """write_log_line prepends ISO timestamp."""
    buffer = StringIO()

    write_log_line(buffer, "test message")

    output = buffer.getvalue()
    assert output.startswith("[")
    assert "] test message\n" in output
    # Verify it's a valid ISO timestamp
    timestamp_str = output[1:output.index("]")]
    datetime.fromisoformat(timestamp_str)


def test_write_log_line_adds_newline():
    """write_log_line adds newline if missing."""
    buffer = StringIO()

    write_log_line(buffer, "no newline")

    assert buffer.getvalue().endswith("\n")


def test_write_log_line_preserves_newline():
    """write_log_line doesn't double newlines."""
    buffer = StringIO()

    write_log_line(buffer, "has newline\n")

    assert buffer.getvalue().endswith("has newline\n")
    assert not buffer.getvalue().endswith("\n\n")


def test_write_log_line_handles_none():
    """write_log_line does nothing when log_file is None."""
    write_log_line(None, "ignored")
    # No exception raised


def test_write_prompt_file_creates_temp_file():
    """write_prompt_file creates a readable temp file."""
    path = write_prompt_file("test prompt content")

    try:
        assert Path(path).exists()
        assert Path(path).read_text() == "test prompt content"
        assert "lf-prompt-" in path
        assert path.endswith(".txt")
    finally:
        os.unlink(path)


def test_write_prompt_file_handles_unicode():
    """write_prompt_file handles unicode content."""
    path = write_prompt_file("café ☕ 日本語")

    try:
        assert Path(path).read_text() == "café ☕ 日本語"
    finally:
        os.unlink(path)
