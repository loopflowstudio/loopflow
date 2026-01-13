"""Tests for loopflow.worktrees wrapper."""

from pathlib import Path
from unittest.mock import MagicMock, patch

import pytest

from loopflow.worktrees import (
    WorktreeError,
    create,
    list_all,
    remove,
)


def test_list_worktrees_maps_json_fields(tmp_path):
    """list_all maps worktrunk JSON into Worktree."""
    repo_root = tmp_path / "repo"
    repo_root.mkdir()

    payload = [
        {
            "branch": "feature",
            "path": str(tmp_path / "repo.feature"),
            "kind": "worktree",
            "working_tree": {
                "staged": True,
                "modified": False,
                "untracked": True,
                "diff_vs_main": {"added": 3, "deleted": 1},
            },
            "main": {"ahead": 2, "behind": 0},
            "remote": {"name": "origin", "branch": "feature", "ahead": 1, "behind": 0},
            "operation_state": "rebase",
            "ci": {"source": "pr", "url": "https://github.com/org/repo/pull/12"},
            "base_branch": "main",
        },
        {
            "branch": "no-worktree",
            "path": str(tmp_path / "repo.no-worktree"),
            "kind": "branch",
        },
    ]

    with patch("subprocess.run") as mock_run:
        mock_run.return_value = MagicMock(returncode=0, stdout=json_dump(payload), stderr="")
        worktrees = list_all(repo_root)

    assert len(worktrees) == 1
    wt = worktrees[0]
    assert wt.branch == "feature"
    assert wt.path == Path(tmp_path / "repo.feature")
    assert wt.has_staged is True
    assert wt.has_untracked is True
    assert wt.is_dirty is True
    assert wt.lines_added == 3
    assert wt.lines_removed == 1
    assert wt.ahead_main == 2
    assert wt.behind_main == 0
    assert wt.ahead_remote == 1
    assert wt.behind_remote == 0
    assert wt.is_rebasing is True
    assert wt.is_merging is False
    assert wt.pr_url == "https://github.com/org/repo/pull/12"
    assert wt.pr_number == 12
    assert wt.base_branch == "main"


def test_create_worktree_returns_existing_path(tmp_path):
    """create returns path from switch when worktree exists."""
    repo_root = tmp_path / "repo"
    repo_root.mkdir()
    existing_path = tmp_path / "repo.feature"

    with patch("loopflow.worktrees.list_all") as mock_list:
        mock_list.return_value = [MagicMock(branch="feature")]
        with patch("subprocess.run") as mock_run:
            mock_run.return_value = MagicMock(returncode=0, stdout=f"{existing_path}\n", stderr="")
            result = create(repo_root, "feature")

    assert result == existing_path


def test_create_worktree_creates_new_path(tmp_path):
    """create returns path for new worktree."""
    repo_root = tmp_path / "repo"
    repo_root.mkdir()
    new_path = tmp_path / "repo.new"

    with patch("loopflow.worktrees.list_all") as mock_list:
        mock_list.return_value = []
        with patch("subprocess.run") as mock_run:
            mock_run.return_value = MagicMock(returncode=0, stdout=f"{new_path}\n", stderr="")
            result = create(repo_root, "new")

    assert result == new_path


def test_create_worktree_missing_wt_raises(tmp_path):
    """Missing wt binary raises WorktreeError."""
    repo_root = tmp_path / "repo"
    repo_root.mkdir()

    with patch("loopflow.worktrees.list_all") as mock_list:
        mock_list.return_value = []
        with patch("subprocess.run", side_effect=FileNotFoundError()):
            with pytest.raises(WorktreeError, match="lf ops install"):
                create(repo_root, "feature")


def test_remove_worktree_returns_false_on_error(tmp_path):
    """remove returns False when worktrunk fails."""
    repo_root = tmp_path / "repo"
    repo_root.mkdir()

    with patch("subprocess.run") as mock_run:
        mock_run.return_value = MagicMock(returncode=1, stdout="", stderr="error")
        assert remove(repo_root, "feature") is False


def json_dump(payload) -> str:
    import json

    return json.dumps(payload)
