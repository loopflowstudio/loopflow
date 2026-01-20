"""Tests for lfops commit command."""

from pathlib import Path
from unittest.mock import patch, MagicMock

from typer.testing import CliRunner

from loopflow.lfops.commands import app


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
    with patch("loopflow.lfops.commit.find_worktree_root", return_value=Path("/fake/repo")):
        with patch("subprocess.run", side_effect=_git_mock(status_output="")):
            result = runner.invoke(app, ["commit"])

            assert result.exit_code == 0
            assert "Nothing to commit" in result.output


def test_commit_with_changes():
    """commit runs agent and commits when there are changes."""
    mock_task = MagicMock()
    mock_task.content = "test commit task"
    mock_runner = MagicMock()
    mock_runner.launch.return_value = MagicMock(exit_code=0)

    with patch("loopflow.lfops.commit.find_worktree_root", return_value=Path("/fake/repo")):
        with patch("subprocess.run", side_effect=_git_mock(status_output="M README.md\n", has_staged=True)):
            with patch("loopflow.lfops.commit.gather_step", return_value=mock_task):
                with patch("loopflow.lfops.commit.gather_prompt_components") as mock_gather:
                    mock_gather.return_value = MagicMock()
                    with patch("loopflow.lfops.commit.format_prompt", return_value="test prompt"):
                        with patch("loopflow.lfops.commit.load_config", return_value=None):
                            with patch("loopflow.lfops.commit.get_runner", return_value=mock_runner):
                                result = runner.invoke(app, ["commit"])

                                assert result.exit_code == 0
                                assert "Committing..." in result.output


def test_commit_with_push_includes_push_output():
    """commit --push pushes and reports success."""
    mock_task = MagicMock()
    mock_task.content = "test commit task"
    mock_runner = MagicMock()
    mock_runner.launch.return_value = MagicMock(exit_code=0)

    with patch("loopflow.lfops.commit.find_worktree_root", return_value=Path("/fake/repo")):
        with patch("loopflow.lfops.commit.has_upstream", return_value=True):
            with patch("subprocess.run", side_effect=_git_mock(status_output="M file.py\n", has_staged=True)):
                with patch("loopflow.lfops.commit.gather_step", return_value=mock_task):
                    with patch("loopflow.lfops.commit.gather_prompt_components") as mock_gather:
                        mock_gather.return_value = MagicMock()
                        with patch("loopflow.lfops.commit.format_prompt", return_value="test prompt"):
                            with patch("loopflow.lfops.commit.load_config", return_value=None):
                                with patch("loopflow.lfops.commit.get_runner", return_value=mock_runner):
                                    result = runner.invoke(app, ["commit", "--push"])

                                    assert result.exit_code == 0
                                    assert "Pushed to origin" in result.output
