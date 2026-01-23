"""Tests for next command."""

from pathlib import Path
from unittest.mock import MagicMock, patch

from loopflow.lfops.next import (
    _enable_auto_merge,
    _get_pr_number,
    _get_pr_state,
    move_worktree,
    next_worktree,
)


def test_get_pr_number_returns_number():
    """Extracts PR number from gh output."""
    with patch("subprocess.run") as mock_run:
        mock_run.return_value = MagicMock(returncode=0, stdout="42\n")
        result = _get_pr_number(Path("/repo"))
    assert result == 42


def test_get_pr_number_returns_none_on_error():
    """Returns None when no PR exists."""
    with patch("subprocess.run") as mock_run:
        mock_run.return_value = MagicMock(returncode=1, stdout="")
        result = _get_pr_number(Path("/repo"))
    assert result is None


def test_get_pr_state_merged():
    """Returns MERGED state."""
    with patch("subprocess.run") as mock_run:
        mock_run.return_value = MagicMock(returncode=0, stdout="MERGED\n")
        result = _get_pr_state(Path("/repo"), 42)
    assert result == "MERGED"


def test_get_pr_state_open():
    """Returns OPEN state."""
    with patch("subprocess.run") as mock_run:
        mock_run.return_value = MagicMock(returncode=0, stdout="OPEN\n")
        result = _get_pr_state(Path("/repo"), 42)
    assert result == "OPEN"


def test_enable_auto_merge_success():
    """Returns True on successful auto-merge enable."""
    with patch("subprocess.run") as mock_run:
        # First call gets PR info, second enables auto-merge
        mock_run.side_effect = [
            MagicMock(returncode=0, stdout='{"title": "My PR", "body": "Description"}'),
            MagicMock(returncode=0),
        ]
        result = _enable_auto_merge(Path("/repo"), 42)
    assert result is True


def test_enable_auto_merge_failure():
    """Returns False when auto-merge fails."""
    with patch("subprocess.run") as mock_run:
        mock_run.return_value = MagicMock(returncode=1)
        result = _enable_auto_merge(Path("/repo"), 42)
    assert result is False


def test_next_fails_on_main_branch():
    """Cannot run next from main branch."""
    with patch("loopflow.lfops.next.find_main_repo", return_value=Path("/repo")):
        with patch("loopflow.lfops.next.get_default_branch", return_value="main"):
            result = next_worktree(Path("/repo"), "main", block=False, open_terminal=False)
    assert result is None


def test_next_fails_without_pr():
    """Fails when no PR exists and create_pr not set."""
    with patch("loopflow.lfops.next.find_main_repo", return_value=Path("/repo")):
        with patch("loopflow.lfops.next.get_default_branch", return_value="main"):
            with patch("loopflow.lfops.next._get_pr_number", return_value=None):
                result = next_worktree(
                    Path("/repo"),
                    "feature-branch",
                    block=False,
                    open_terminal=False,
                    create_pr=False,
                )
    assert result is None


def test_move_worktree_success():
    """move_worktree removes and recreates at same path."""
    with patch("subprocess.run") as mock_run:
        mock_run.return_value = MagicMock(returncode=0)
        result = move_worktree(
            Path("/main"),
            Path("/main/worktree"),
            "new-branch",
            "main",
        )
    assert result is True
    # Should call: remove, add, push
    assert mock_run.call_count == 3


def test_move_worktree_fails_on_remove():
    """move_worktree returns False if remove fails."""
    with patch("subprocess.run") as mock_run:
        mock_run.return_value = MagicMock(returncode=1)
        result = move_worktree(
            Path("/main"),
            Path("/main/worktree"),
            "new-branch",
            "main",
        )
    assert result is False


def test_next_moves_worktree_in_place(tmp_path):
    """next_worktree returns same path (moves in place)."""
    repo = tmp_path / "repo"
    repo.mkdir()

    with patch("loopflow.lfops.next.find_main_repo", return_value=repo):
        with patch("loopflow.lfops.next.get_default_branch", return_value="main"):
            with patch("loopflow.lfops.next._get_pr_number", return_value=42):
                with patch("loopflow.lfops.next._enable_auto_merge", return_value=True):
                    with patch(
                        "loopflow.lfops.next.generate_next_branch",
                        return_value="jack.auth.20260123_1112-aurora-melody",
                    ):
                        with patch(
                            "loopflow.lfops.next.parse_branch_base",
                            return_value="jack.auth.20260123_1112",
                        ):
                            with patch(
                                "loopflow.lfops.next.move_worktree", return_value=True
                            ):
                                with patch("loopflow.lfops.next.write_directive"):
                                    result = next_worktree(
                                        repo,
                                        "jack.auth.20260123_1112",
                                        block=False,
                                        open_terminal=False,
                                    )

    assert result is not None
    # Returns same path - worktree moved in place
    assert result == repo
