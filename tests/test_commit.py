"""Tests for lfpr commit command."""

from pathlib import Path
from unittest.mock import patch, MagicMock

from typer.testing import CliRunner

from loopflow.lfpr import app


runner = CliRunner()


def _git_mock(status_output="", has_staged=False, commit_ok=True, push_ok=True):
    """Create a subprocess mock that responds to git commands by inspecting args."""
    def side_effect(cmd, **kwargs):
        if cmd[0] != "git":
            return MagicMock(returncode=0)

        if cmd[1:3] == ["status", "--porcelain"]:
            return MagicMock(returncode=0, stdout=status_output)

        if cmd[1:3] == ["add", "-A"]:
            return MagicMock(returncode=0)

        if cmd[1:3] == ["diff", "--cached"]:
            # returncode 1 = there are staged changes
            return MagicMock(returncode=1 if has_staged else 0)

        if cmd[1] == "commit":
            return MagicMock(returncode=0 if commit_ok else 1)

        if cmd[1] == "push":
            return MagicMock(returncode=0 if push_ok else 1)

        if cmd[1:3] == ["branch", "--show-current"]:
            return MagicMock(returncode=0, stdout="feature-branch\n")

        return MagicMock(returncode=0)

    return side_effect


def test_commit_with_no_changes():
    """commit exits cleanly when there's nothing to commit."""
    with patch("loopflow.lfpr.find_worktree_root", return_value=Path("/fake/repo")):
        with patch("subprocess.run", side_effect=_git_mock(status_output="")):
            result = runner.invoke(app, ["commit"])

            assert result.exit_code == 0
            assert "Nothing to commit" in result.output


def test_commit_with_changes():
    """commit generates message and commits when there are changes."""
    with patch("loopflow.lfpr.find_worktree_root", return_value=Path("/fake/repo")):
        with patch("subprocess.run", side_effect=_git_mock(status_output="M README.md\n", has_staged=True)):
            with patch("loopflow.lfpr.generate_commit_message") as mock_gen:
                mock_gen.return_value = MagicMock(title="fix: typo", body="Fixed typo")

                result = runner.invoke(app, ["commit"])

                assert result.exit_code == 0
                assert "Committed: fix: typo" in result.output


def test_commit_with_custom_message_skips_generation():
    """commit uses provided message instead of calling LLM."""
    # Note: lfpr commit doesn't have a -m flag, this test is now obsolete
    pass


def test_commit_with_push_includes_push_output():
    """commit --push pushes and reports success."""
    with patch("loopflow.lfpr.find_worktree_root", return_value=Path("/fake/repo")):
        with patch("loopflow.lfpr.has_upstream", return_value=True):
            with patch("subprocess.run", side_effect=_git_mock(status_output="M file.py\n", has_staged=True)):
                with patch("loopflow.lfpr.generate_commit_message") as mock_gen:
                    mock_gen.return_value = MagicMock(title="fix: bug", body=None)

                    result = runner.invoke(app, ["commit", "--push"])

                    assert result.exit_code == 0
                    assert "Pushed to origin" in result.output
