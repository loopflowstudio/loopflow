"""Tests for lf ops commit command."""

from pathlib import Path
from unittest.mock import MagicMock, patch

from typer.testing import CliRunner

from loopflow.lf.cli import app

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
    with patch("loopflow.lf.ops.commit.find_worktree_root", return_value=Path("/fake/repo")):
        with patch("subprocess.run", side_effect=_git_mock(status_output="")):
            result = runner.invoke(app, ["ops", "commit"])

            assert result.exit_code == 0
            assert "Nothing to commit" in result.output


def test_commit_with_changes():
    """commit runs agent and commits when there are changes."""
    mock_task = MagicMock()
    mock_task.content = "test commit task"
    mock_runner = MagicMock()
    mock_runner.launch.return_value = MagicMock(exit_code=0)

    git_mock = _git_mock(status_output="M README.md\n", has_staged=True)
    with (
        patch("loopflow.lf.ops.commit.find_worktree_root", return_value=Path("/fake/repo")),
        patch("subprocess.run", side_effect=git_mock),
        patch("loopflow.lf.ops.commit.gather_step", return_value=mock_task),
        patch("loopflow.lf.ops.commit.gather_prompt_components") as mock_gather,
        patch("loopflow.lf.ops.commit.format_prompt", return_value="test prompt"),
        patch("loopflow.lf.ops.commit.load_config", return_value=None),
        patch("loopflow.lf.ops.commit.get_runner", return_value=mock_runner),
    ):
        mock_gather.return_value = MagicMock()
        result = runner.invoke(app, ["ops", "commit"])

        assert result.exit_code == 0
        assert "Committing..." in result.output


def test_commit_with_push_includes_push_output():
    """commit --push pushes and reports success."""
    mock_task = MagicMock()
    mock_task.content = "test commit task"
    mock_runner = MagicMock()
    mock_runner.launch.return_value = MagicMock(exit_code=0)

    git_mock = _git_mock(status_output="M file.py\n", has_staged=True)
    with (
        patch("loopflow.lf.ops.commit.find_worktree_root", return_value=Path("/fake/repo")),
        patch("loopflow.lf.ops.commit.has_upstream", return_value=True),
        patch("subprocess.run", side_effect=git_mock),
        patch("loopflow.lf.ops.commit.gather_step", return_value=mock_task),
        patch("loopflow.lf.ops.commit.gather_prompt_components") as mock_gather,
        patch("loopflow.lf.ops.commit.format_prompt", return_value="test prompt"),
        patch("loopflow.lf.ops.commit.load_config", return_value=None),
        patch("loopflow.lf.ops.commit.get_runner", return_value=mock_runner),
    ):
        mock_gather.return_value = MagicMock()
        result = runner.invoke(app, ["ops", "commit", "--push"])

        assert result.exit_code == 0
        assert "Pushed to origin" in result.output
