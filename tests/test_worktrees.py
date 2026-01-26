"""Tests for loopflow.worktrees wrapper."""

from pathlib import Path
from unittest.mock import MagicMock, patch

import pytest

from loopflow.lf.worktrees import (
    Worktree,
    WorktreeError,
    create,
    find_merged,
    is_merged,
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

    with patch("loopflow.lf.worktrees.list_all") as mock_list:
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

    with patch("loopflow.lf.worktrees.list_all") as mock_list:
        mock_list.return_value = []
        with patch("subprocess.run") as mock_run:
            mock_run.return_value = MagicMock(returncode=0, stdout=f"{new_path}\n", stderr="")
            result = create(repo_root, "new")

    assert result == new_path


def test_create_worktree_missing_wt_raises(tmp_path):
    """Missing wt binary raises WorktreeError."""
    repo_root = tmp_path / "repo"
    repo_root.mkdir()

    with patch("loopflow.lf.worktrees.list_all") as mock_list:
        mock_list.return_value = []
        with patch("loopflow.lf.worktrees._find_wt_binary", return_value=None):
            with pytest.raises(WorktreeError, match="lfd install"):
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


def _make_worktree(
    branch: str,
    is_dirty: bool = False,
    pr_state: str | None = None,
    ahead_main: int = 0,
) -> Worktree:
    """Create a Worktree for testing."""
    return Worktree(
        name=branch,
        path=Path(f"/tmp/{branch}"),
        branch=branch,
        base_branch="main",
        on_origin=True,
        is_dirty=is_dirty,
        main_state=None,
        integration_reason=None,
        pr_url=None,
        pr_number=None,
        pr_state=pr_state,
        ahead_main=ahead_main,
        behind_main=0,
        ahead_remote=0,
        behind_remote=0,
        lines_added=0,
        lines_removed=0,
        has_staged=False,
        has_modified=is_dirty,
        has_untracked=False,
        is_rebasing=False,
        is_merging=False,
    )


def test_is_merged_returns_false_for_main():
    """Never consider main/master as merged."""
    wt = _make_worktree("main")
    assert is_merged(wt, Path("/tmp/repo")) is False

    wt = _make_worktree("master")
    assert is_merged(wt, Path("/tmp/repo")) is False


def test_is_merged_returns_false_for_dirty_worktree():
    """Dirty worktrees are not considered merged."""
    wt = _make_worktree("feature", is_dirty=True, pr_state="merged")
    assert is_merged(wt, Path("/tmp/repo")) is False


def test_is_merged_returns_true_when_pr_state_merged():
    """PR state 'merged' means the worktree is merged."""
    wt = _make_worktree("feature", pr_state="merged")
    # No subprocess call needed when pr_state is merged
    assert is_merged(wt, Path("/tmp/repo")) is True


def test_is_merged_checks_ancestor_for_squash_merge(tmp_path):
    """Checks if branch is ancestor of origin/main for squash merges."""
    # Branch has commits ahead of main (before being squash-merged)
    wt = _make_worktree("feature", pr_state=None, ahead_main=3)

    # Branch is ancestor of origin/main (squash merged)
    with patch("loopflow.lf.worktrees.get_pr_state", return_value=None):
        with patch("subprocess.run") as mock_run:
            mock_run.return_value = MagicMock(returncode=0)  # is-ancestor returns 0 when true
            result = is_merged(wt, tmp_path)

    assert result is True


def test_is_merged_returns_false_when_not_ancestor(tmp_path):
    """Returns false when branch is not ancestor of origin/main."""
    # Branch has commits ahead of main (active work)
    wt = _make_worktree("feature", pr_state=None, ahead_main=3)

    # Branch is not an ancestor (not yet merged)
    with patch("loopflow.lf.worktrees.get_pr_state", return_value=None):
        with patch("subprocess.run") as mock_run:
            mock_run.return_value = MagicMock(returncode=1)  # is-ancestor returns 1 when false
            result = is_merged(wt, tmp_path)

    assert result is False


def test_is_merged_returns_false_for_new_branch_with_no_commits():
    """New branches with no commits ahead should not be considered merged."""
    # New branch just created from main - no commits, no PR
    wt = _make_worktree("new-feature", pr_state=None, ahead_main=0)

    # Should return False without any subprocess calls
    result = is_merged(wt, Path("/tmp/repo"))
    assert result is False


def test_find_merged_returns_only_merged_worktrees(tmp_path):
    """find_merged filters to only merged worktrees."""
    merged_wt = _make_worktree("merged-feature", pr_state="merged")
    unmerged_wt = _make_worktree("unmerged-feature", pr_state="open")
    main_wt = _make_worktree("main")

    with patch("loopflow.lf.worktrees.list_all") as mock_list:
        mock_list.return_value = [merged_wt, unmerged_wt, main_wt]
        with patch("subprocess.run") as mock_run:
            # unmerged returns non-zero (not ancestor)
            mock_run.return_value = MagicMock(returncode=1)
            result = find_merged(tmp_path)

    # Only the merged worktree should be returned
    assert len(result) == 1
    assert result[0].branch == "merged-feature"
