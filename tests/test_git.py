"""Tests for loopflow.git module."""

import subprocess
from pathlib import Path
from unittest.mock import patch, MagicMock

import pytest

from loopflow.git import GitError, autocommit, create_worktree, find_main_repo, open_pr


@pytest.fixture
def temp_repo(tmp_path):
    """Create a minimal repo."""
    (tmp_path / ".git").mkdir()
    (tmp_path / ".lf").mkdir()
    return tmp_path


def test_create_worktree_raises_on_existing_branch(temp_repo):
    """Creating worktree with existing branch raises GitError."""
    with patch("subprocess.run") as mock_run:
        mock_run.return_value = MagicMock(
            returncode=1,
            stderr="fatal: a branch named 'feature' already exists"
        )

        with pytest.raises(GitError, match="already exists"):
            create_worktree(temp_repo, "feature")


def test_create_worktree_raises_on_failure(temp_repo):
    """Creating worktree that fails raises GitError."""
    with patch("subprocess.run") as mock_run:
        mock_run.return_value = MagicMock(
            returncode=1,
            stderr="fatal: some other error"
        )

        with pytest.raises(GitError, match="some other error"):
            create_worktree(temp_repo, "feature")


def test_create_worktree_returns_existing_path(temp_repo):
    """Creating worktree for existing path returns it without error."""
    worktree_path = temp_repo / ".lf" / "worktrees" / "feature"
    worktree_path.mkdir(parents=True)

    result = create_worktree(temp_repo, "feature")
    assert result == worktree_path


def test_open_pr_raises_on_failure(temp_repo):
    """Opening PR that fails raises GitError."""
    with patch("subprocess.run") as mock_run:
        # Mock push success
        mock_run.return_value = MagicMock(returncode=0)

        # Mock gh pr create failure
        def side_effect(*args, **kwargs):
            if "gh" in args[0]:
                return MagicMock(
                    returncode=1,
                    stderr="error: failed to create PR",
                    stdout=""
                )
            return MagicMock(returncode=0)

        mock_run.side_effect = side_effect

        with pytest.raises(GitError, match="failed to create PR"):
            open_pr(temp_repo)


def test_open_pr_returns_existing_pr_url(temp_repo):
    """Opening PR when one exists returns existing URL."""
    with patch("subprocess.run") as mock_run:
        def side_effect(*args, **kwargs):
            if "gh" in args[0] and "create" in args[0]:
                return MagicMock(
                    returncode=1,
                    stderr="already exists",
                    stdout=""
                )
            if "gh" in args[0] and "view" in args[0]:
                return MagicMock(
                    returncode=0,
                    stdout="https://github.com/user/repo/pull/1"
                )
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

        result = autocommit(temp_repo, "test", arg="foo.md")

        assert result is True
        # Verify git add and git commit were called
        assert mock_run.call_count >= 3

        # Check commit message includes task and arg
        # Find the actual git commit call (not just containing "commit" substring)
        commit_calls = [c for c in mock_run.call_args_list if len(c[0]) > 0 and c[0][0][0] == "git" and "commit" in c[0][0]]
        assert len(commit_calls) > 0
        # The commit message should be in the -m flag
        commit_args = commit_calls[0][0][0]
        assert "lf test foo.md" in " ".join(commit_args)


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
        mock_run.return_value = MagicMock(
            returncode=0,
            stdout=str(temp_repo / ".git")
        )

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
        mock_run.return_value = MagicMock(
            returncode=0,
            stdout=str(main_repo / ".git")
        )

        result = find_main_repo(worktree)
        assert result == main_repo


def test_find_main_repo_not_in_git_repo(tmp_path):
    """find_main_repo returns None when not in a git repo."""
    with patch("subprocess.run") as mock_run:
        mock_run.return_value = MagicMock(returncode=128, stdout="", stderr="fatal: not a git repository")

        result = find_main_repo(tmp_path)
        assert result is None
