"""Tests for loopflow.launcher runner abstraction."""

import pytest
from unittest.mock import patch, MagicMock

from loopflow.launcher import get_runner, ClaudeRunner, CodexRunner, LaunchResult


def test_get_runner_returns_claude():
    """get_runner returns ClaudeRunner instance for 'claude'."""
    runner = get_runner("claude")
    assert isinstance(runner, ClaudeRunner)


def test_get_runner_returns_codex():
    """get_runner returns CodexRunner instance for 'codex'."""
    runner = get_runner("codex")
    assert isinstance(runner, CodexRunner)


def test_get_runner_raises_on_unknown():
    """get_runner raises ValueError for unknown model name."""
    with pytest.raises(ValueError, match="Unknown model"):
        get_runner("gpt4")


def test_launch_result_dataclass():
    """LaunchResult holds exit code and optional output."""
    result = LaunchResult(exit_code=0, output="success")
    assert result.exit_code == 0
    assert result.output == "success"

    result_no_output = LaunchResult(exit_code=1)
    assert result_no_output.exit_code == 1
    assert result_no_output.output is None


def test_claude_runner_is_available():
    """ClaudeRunner.is_available() checks for claude CLI."""
    runner = ClaudeRunner()

    with patch("loopflow.launcher.check_claude_available") as mock_check:
        mock_check.return_value = True
        assert runner.is_available() is True

        mock_check.return_value = False
        assert runner.is_available() is False


def test_codex_runner_is_available():
    """CodexRunner.is_available() checks for codex CLI."""
    runner = CodexRunner()

    with patch("subprocess.run") as mock_run:
        # CLI exists
        mock_run.return_value = MagicMock(returncode=0)
        assert runner.is_available() is True

        # CLI doesn't exist
        mock_run.side_effect = FileNotFoundError()
        assert runner.is_available() is False


def test_claude_runner_launch_returns_result():
    """ClaudeRunner.launch() returns LaunchResult."""
    runner = ClaudeRunner()

    with patch("loopflow.launcher.launch_claude") as mock_launch:
        mock_launch.return_value = (0, "output")
        result = runner.launch("test prompt")

        assert isinstance(result, LaunchResult)
        assert result.exit_code == 0
        assert result.output == "output"


def test_codex_runner_launch_auto_mode():
    """CodexRunner.launch() with auto captures output."""
    runner = CodexRunner()

    with patch("subprocess.run") as mock_run:
        mock_run.return_value = MagicMock(returncode=0, stdout="codex output")
        result = runner.launch("test prompt", auto=True)

        assert result.exit_code == 0
        assert result.output == "codex output"


def test_codex_runner_launch_interactive():
    """CodexRunner.launch() without auto runs interactively."""
    runner = CodexRunner()

    with patch("subprocess.run") as mock_run:
        mock_run.return_value = MagicMock(returncode=0)
        result = runner.launch("test prompt", auto=False)

        assert result.exit_code == 0
        assert result.output is None
