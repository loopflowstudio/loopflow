"""Tests for loopflow.git module."""

from unittest.mock import MagicMock, patch

import pytest

from loopflow.lf.git import GitError, autocommit, find_main_repo, get_pr_target, open_pr


@pytest.fixture
def temp_repo(tmp_path):
    """Create a minimal repo."""
    (tmp_path / ".git").mkdir()
    (tmp_path / ".lf").mkdir()
    return tmp_path


def test_open_pr_raises_on_failure(temp_repo):
    """Opening PR that fails raises GitError."""
    with patch("subprocess.run") as mock_run:
        # Mock push success
        mock_run.return_value = MagicMock(returncode=0)

        # Mock gh pr create failure
        def side_effect(*args, **kwargs):
            if "gh" in args[0]:
                return MagicMock(returncode=1, stderr="error: failed to create PR", stdout="")
            return MagicMock(returncode=0)

        mock_run.side_effect = side_effect

        with pytest.raises(GitError, match="failed to create PR"):
            open_pr(temp_repo)


def test_open_pr_returns_existing_pr_url(temp_repo):
    """Opening PR when one exists returns existing URL."""
    with patch("subprocess.run") as mock_run:

        def side_effect(*args, **kwargs):
            if "gh" in args[0] and "create" in args[0]:
                return MagicMock(returncode=1, stderr="already exists", stdout="")
            if "gh" in args[0] and "view" in args[0]:
                return MagicMock(returncode=0, stdout="https://github.com/user/repo/pull/1")
            return MagicMock(returncode=0)

        mock_run.side_effect = side_effect

        url = open_pr(temp_repo)
        assert url == "https://github.com/user/repo/pull/1"


def test_autocommit_returns_false_when_no_changes(temp_repo):
    """autocommit returns False when working tree is clean."""
    with patch("subprocess.run") as mock_run:
        mock_run.return_value = MagicMock(returncode=0, stdout="")

        result = autocommit(temp_repo, "test")
        assert result is False


def test_autocommit_creates_commit_when_dirty(temp_repo):
    """autocommit creates commit and returns True when changes exist."""
    with patch("subprocess.run") as mock_run:
        call_count = [0]

        def side_effect(*args, **kwargs):
            call_count[0] += 1
            if call_count[0] == 1:  # git status
                return MagicMock(returncode=0, stdout="M file.py\n")
            return MagicMock(returncode=0)

        mock_run.side_effect = side_effect

        result = autocommit(temp_repo, "test")

        assert result is True
        # Verify git add and git commit were called
        assert mock_run.call_count >= 3

        # Check commit message includes task
        # Find the actual git commit call (not just containing "commit" substring)
        commit_calls = [
            c
            for c in mock_run.call_args_list
            if len(c[0]) > 0 and c[0][0][0] == "git" and "commit" in c[0][0]
        ]
        assert len(commit_calls) > 0
        # The commit message should be in the -m flag
        commit_args = commit_calls[0][0][0]
        assert "lf test" in " ".join(commit_args)


def test_autocommit_pushes_when_flag_set(temp_repo):
    """autocommit pushes to origin when push=True and upstream exists."""
    with patch("subprocess.run") as mock_run:
        call_count = [0]

        def side_effect(*args, **kwargs):
            call_count[0] += 1
            if call_count[0] == 1:  # git status --porcelain
                return MagicMock(returncode=0, stdout="M file.py\n")
            if "rev-parse" in args[0]:  # has_upstream check
                return MagicMock(returncode=0)
            if "push" in args[0]:
                return MagicMock(returncode=0)
            return MagicMock(returncode=0)

        mock_run.side_effect = side_effect

        result = autocommit(temp_repo, "test", push=True, verbose=False)

        assert result is True
        # Verify push was called
        push_calls = [c for c in mock_run.call_args_list if "push" in str(c)]
        assert len(push_calls) > 0


def test_find_main_repo_from_main_repo(temp_repo):
    """find_main_repo returns repo root when in main repo."""
    with patch("subprocess.run") as mock_run:
        # Simulate being in a regular repo (not a worktree)
        mock_run.return_value = MagicMock(returncode=0, stdout=str(temp_repo / ".git"))

        result = find_main_repo(temp_repo)
        assert result == temp_repo


def test_find_main_repo_from_worktree(tmp_path):
    """find_main_repo returns main repo root when in a worktree."""
    main_repo = tmp_path / "main"
    main_repo.mkdir()
    (main_repo / ".git").mkdir()

    worktree = tmp_path / "worktree"
    worktree.mkdir()

    with patch("subprocess.run") as mock_run:
        # Simulate git returning the main repo's .git dir
        mock_run.return_value = MagicMock(returncode=0, stdout=str(main_repo / ".git"))

        result = find_main_repo(worktree)
        assert result == main_repo


def test_find_main_repo_not_in_git_repo(tmp_path):
    """find_main_repo returns None when not in a git repo."""
    with patch("subprocess.run") as mock_run:
        mock_run.return_value = MagicMock(
            returncode=128, stdout="", stderr="fatal: not a git repository"
        )

        result = find_main_repo(tmp_path)
        assert result is None


def test_get_current_branch_returns_branch_name(temp_repo):
    """get_current_branch returns current branch name."""
    from loopflow.lf.git import get_current_branch

    with patch("subprocess.run") as mock_run:
        mock_run.return_value = MagicMock(returncode=0, stdout="feature-branch\n")

        result = get_current_branch(temp_repo)
        assert result == "feature-branch"


def test_get_current_branch_returns_none_when_detached(temp_repo):
    """get_current_branch returns None when HEAD is detached."""
    from loopflow.lf.git import get_current_branch

    with patch("subprocess.run") as mock_run:
        mock_run.return_value = MagicMock(returncode=0, stdout="")

        result = get_current_branch(temp_repo)
        assert result is None


# PR targeting tests for stacked worktrees


def test_get_pr_target_returns_main_when_no_base():
    """get_pr_target returns 'main' when base_branch is None."""
    assert get_pr_target(None) == "main"


def test_get_pr_target_returns_main_when_base_is_main():
    """get_pr_target returns 'main' when base_branch is 'main'."""
    assert get_pr_target("main") == "main"


def test_get_pr_target_returns_base_when_pr_open():
    """get_pr_target returns base_branch when base PR is still open."""
    with patch("subprocess.run") as mock_run:
        mock_run.return_value = MagicMock(returncode=0, stdout="OPEN\n")

        result = get_pr_target("feature-A")
        assert result == "feature-A"


def test_get_pr_target_returns_main_when_base_pr_merged():
    """get_pr_target returns 'main' when base PR has merged."""
    with patch("subprocess.run") as mock_run:
        mock_run.return_value = MagicMock(returncode=0, stdout="MERGED\n")

        result = get_pr_target("feature-A")
        assert result == "main"


def test_get_pr_target_returns_base_when_gh_fails():
    """get_pr_target returns base_branch when gh pr view fails."""
    with patch("subprocess.run") as mock_run:
        mock_run.return_value = MagicMock(returncode=1, stdout="", stderr="not found")

        result = get_pr_target("feature-A")
        assert result == "feature-A"
