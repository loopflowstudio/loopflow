"""Tests for lf ops commit command."""

from pathlib import Path
from unittest.mock import patch, MagicMock

from typer.testing import CliRunner

from loopflow.cli import app


runner = CliRunner()


def test_commit_with_no_changes():
    """commit exits cleanly when there's nothing to commit."""
    with patch("loopflow.cli.commit.find_worktree_root", return_value=Path("/fake/repo")):
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
                    if call_count[0] == 1:  # git status --porcelain
                        return MagicMock(returncode=0, stdout="M README.md\n")
                    if call_count[0] == 3:  # git diff --cached --quiet (returncode 1 = has staged)
                        return MagicMock(returncode=1)
                    return MagicMock(returncode=0)

                mock_run.side_effect = side_effect

                result = runner.invoke(app, ["ops", "commit"])

                assert result.exit_code == 0
                assert "Generating commit message" in result.output
                assert "Committed: fix: typo" in result.output


def test_commit_with_custom_message():
    """commit uses provided message instead of generating one."""
    mock_repo = Path("/fake/repo")

    with patch("loopflow.cli.commit.find_worktree_root", return_value=mock_repo):
        with patch("subprocess.run") as mock_run:
            call_count = [0]
            def side_effect(*args, **kwargs):
                call_count[0] += 1
                if call_count[0] == 1:  # git status --porcelain
                    return MagicMock(returncode=0, stdout="M README.md\n")
                if call_count[0] == 3:  # git diff --cached --quiet (returncode 1 = has staged)
                    return MagicMock(returncode=1)
                return MagicMock(returncode=0)

            mock_run.side_effect = side_effect

            result = runner.invoke(app, ["ops", "commit", "-m", "my custom message"])

            assert result.exit_code == 0
            assert "Committed: my custom message" in result.output
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
                        if call_count[0] == 1:  # git status --porcelain
                            return MagicMock(returncode=0, stdout="M file.py\n")
                        if call_count[0] == 3:  # git diff --cached --quiet (returncode 1 = has staged)
                            return MagicMock(returncode=1)
                        return MagicMock(returncode=0)

                    mock_run.side_effect = side_effect

                    result = runner.invoke(app, ["ops", "commit", "--push"])

                    assert result.exit_code == 0
                    assert "Pushed to origin" in result.output
