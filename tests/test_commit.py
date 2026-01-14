"""Tests for lf ops commit command."""

from pathlib import Path
from unittest.mock import patch, MagicMock

import pytest
from typer.testing import CliRunner

from loopflow.cli import app


runner = CliRunner()


@pytest.fixture
def mock_repo(tmp_path):
    """Create a minimal repo directory."""
    (tmp_path / ".git").mkdir()
    return tmp_path


def test_commit_with_no_changes(mock_repo):
    """commit exits cleanly when there's nothing to commit."""
    with patch("loopflow.cli.commit.find_worktree_root", return_value=mock_repo):
        with patch("subprocess.run") as mock_run:
            mock_run.return_value = MagicMock(returncode=0, stdout="")

            result = runner.invoke(app, ["ops", "commit"])

            assert result.exit_code == 0
            assert "Nothing to commit" in result.output


def test_commit_with_changes():
    """commit stages, generates message, and commits."""
    mock_repo = Path("/fake/repo")

    with patch("loopflow.cli.commit.find_worktree_root", return_value=mock_repo):
        with patch("subprocess.run") as mock_run:
            with patch("loopflow.cli.commit.generate_commit_message") as mock_gen:
                mock_gen.return_value = MagicMock(title="fix: typo", body="Fixed typo in README")

                call_count = [0]
                def side_effect(*args, **kwargs):
                    call_count[0] += 1
                    if call_count[0] == 1:  # git status
                        return MagicMock(returncode=0, stdout="M README.md\n")
                    return MagicMock(returncode=0)

                mock_run.side_effect = side_effect

                result = runner.invoke(app, ["ops", "commit"])

                assert result.exit_code == 0
                assert "Staging changes" in result.output
                assert "Generating commit message" in result.output
                assert "Committing: fix: typo" in result.output


def test_commit_with_custom_message():
    """commit uses provided message instead of generating one."""
    mock_repo = Path("/fake/repo")

    with patch("loopflow.cli.commit.find_worktree_root", return_value=mock_repo):
        with patch("subprocess.run") as mock_run:
            call_count = [0]
            def side_effect(*args, **kwargs):
                call_count[0] += 1
                if call_count[0] == 1:  # git status
                    return MagicMock(returncode=0, stdout="M README.md\n")
                return MagicMock(returncode=0)

            mock_run.side_effect = side_effect

            result = runner.invoke(app, ["ops", "commit", "-m", "my custom message"])

            assert result.exit_code == 0
            assert "Committing: my custom message" in result.output
            assert "Generating commit message" not in result.output


def test_commit_with_push():
    """commit pushes when --push flag is set."""
    mock_repo = Path("/fake/repo")

    with patch("loopflow.cli.commit.find_worktree_root", return_value=mock_repo):
        with patch("loopflow.cli.commit.has_upstream", return_value=True):
            with patch("subprocess.run") as mock_run:
                with patch("loopflow.cli.commit.generate_commit_message") as mock_gen:
                    mock_gen.return_value = MagicMock(title="fix: bug", body=None)

                    call_count = [0]
                    def side_effect(*args, **kwargs):
                        call_count[0] += 1
                        if call_count[0] == 1:  # git status
                            return MagicMock(returncode=0, stdout="M file.py\n")
                        return MagicMock(returncode=0)

                    mock_run.side_effect = side_effect

                    result = runner.invoke(app, ["ops", "commit", "--push"])

                    assert result.exit_code == 0
                    assert "Pushing" in result.output
