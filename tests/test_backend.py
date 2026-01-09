"""Tests for loopflow.launcher backend abstraction."""

import pytest
from unittest.mock import patch, MagicMock

from loopflow.launcher import get_backend, ClaudeBackend, CodexBackend, LaunchResult


def test_get_backend_returns_claude():
    """get_backend returns ClaudeBackend instance for 'claude'."""
    backend = get_backend("claude")
    assert isinstance(backend, ClaudeBackend)


def test_get_backend_returns_codex():
    """get_backend returns CodexBackend instance for 'codex'."""
    backend = get_backend("codex")
    assert isinstance(backend, CodexBackend)


def test_get_backend_raises_on_unknown():
    """get_backend raises ValueError for unknown backend name."""
    with pytest.raises(ValueError, match="Unknown backend"):
        get_backend("gpt4")


def test_launch_result_dataclass():
    """LaunchResult holds exit code and optional output."""
    result = LaunchResult(exit_code=0, output="success")
    assert result.exit_code == 0
    assert result.output == "success"

    result_no_output = LaunchResult(exit_code=1)
    assert result_no_output.exit_code == 1
    assert result_no_output.output is None


def test_claude_backend_is_available():
    """ClaudeBackend.is_available() checks for claude CLI."""
    backend = ClaudeBackend()

    with patch("loopflow.launcher.check_claude_available") as mock_check:
        mock_check.return_value = True
        assert backend.is_available() is True

        mock_check.return_value = False
        assert backend.is_available() is False


def test_codex_backend_is_available():
    """CodexBackend.is_available() checks for codex CLI."""
    backend = CodexBackend()

    with patch("subprocess.run") as mock_run:
        # CLI exists
        mock_run.return_value = MagicMock(returncode=0)
        assert backend.is_available() is True

        # CLI doesn't exist
        mock_run.side_effect = FileNotFoundError()
        assert backend.is_available() is False


def test_claude_backend_launch_returns_result():
    """ClaudeBackend.launch() returns LaunchResult."""
    backend = ClaudeBackend()

    with patch("loopflow.launcher.launch_claude") as mock_launch:
        mock_launch.return_value = (0, "output")
        result = backend.launch("test prompt")

        assert isinstance(result, LaunchResult)
        assert result.exit_code == 0
        assert result.output == "output"


def test_codex_backend_launch_print_mode():
    """CodexBackend.launch() with print_mode captures output."""
    backend = CodexBackend()

    with patch("subprocess.run") as mock_run:
        mock_run.return_value = MagicMock(returncode=0, stdout="codex output")
        result = backend.launch("test prompt", print_mode=True)

        assert result.exit_code == 0
        assert result.output == "codex output"


def test_codex_backend_launch_interactive():
    """CodexBackend.launch() without print_mode runs interactively."""
    backend = CodexBackend()

    with patch("subprocess.run") as mock_run:
        mock_run.return_value = MagicMock(returncode=0)
        result = backend.launch("test prompt", print_mode=False)

        assert result.exit_code == 0
        assert result.output is None
