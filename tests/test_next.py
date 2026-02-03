"""Tests for next command."""

from pathlib import Path
from unittest.mock import MagicMock, patch

import pytest

from loopflow.lf.ops.next import (
    _enable_auto_merge,
    _get_pr_number,
    _get_pr_state,
    next_iteration,
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
    mock_message = MagicMock(title="My PR", body="Description")
    with patch("loopflow.lf.ops.next.generate_pr_message", return_value=mock_message):
        with patch("subprocess.run") as mock_run:
            mock_run.side_effect = [
                MagicMock(returncode=0),  # gh pr edit
                MagicMock(returncode=0),  # gh pr merge --auto
            ]
            result = _enable_auto_merge(Path("/repo"), 42)
    assert result is True


def test_enable_auto_merge_failure():
    """Returns False when auto-merge fails."""
    mock_message = MagicMock(title="My PR", body="Description")
    with patch("loopflow.lf.ops.next.generate_pr_message", return_value=mock_message):
        with patch("subprocess.run") as mock_run:
            mock_run.side_effect = [
                MagicMock(returncode=0),  # gh pr edit
                MagicMock(returncode=1),  # gh pr merge --auto fails
            ]
            result = _enable_auto_merge(Path("/repo"), 42)
    assert result is False


def test_next_fails_without_wave():
    """Fails when no wave exists for worktree."""
    with patch("loopflow.lf.ops.next.get_wave_by_worktree", return_value=None):
        result = next_iteration(Path("/repo"))
    assert result is None


def test_next_fails_on_main_branch():
    """Cannot run next from main branch."""
    mock_wave = MagicMock(name="test-wave", id="wave-123")
    with patch("loopflow.lf.ops.next.get_wave_by_worktree", return_value=mock_wave):
        with patch("loopflow.lf.ops.next.find_main_repo", return_value=Path("/repo")):
            with patch("loopflow.lf.ops.next.get_default_branch", return_value="main"):
                with patch("loopflow.lf.ops.next.get_current_branch", return_value="main"):
                    result = next_iteration(Path("/repo"))
    assert result is None


def test_next_creates_branch_from_wave_name(tmp_path):
    """Creates new branch using wave name."""
    mock_wave = MagicMock()
    mock_wave.name = "concerto"
    mock_wave.id = "wave-123"

    with patch("loopflow.lf.ops.next.get_wave_by_worktree", return_value=mock_wave):
        with patch("loopflow.lf.ops.next.find_main_repo", return_value=tmp_path):
            with patch("loopflow.lf.ops.next.get_default_branch", return_value="main"):
                with patch("loopflow.lf.ops.next.get_current_branch", return_value="feature"):
                    with patch("loopflow.lf.ops.next._rebase_onto_main", return_value=True):
                        with patch("loopflow.lf.ops.next._get_pr_number", return_value=None):
                            with patch(
                                "loopflow.lf.ops.next.generate_branch_name",
                                return_value="jack.concerto.20260202_1700.vale-tempo",
                            ) as mock_gen:
                                with patch("loopflow.lf.ops.next.git_create_branch"):
                                    with patch("subprocess.run"):
                                        with patch("loopflow.lf.ops.next.update_wave_worktree_branch"):
                                            result = next_iteration(tmp_path, rebase=True)

    # Should call generate_branch_name with wave name
    mock_gen.assert_called_once_with("concerto", tmp_path)
    assert result == "jack.concerto.20260202_1700.vale-tempo"


def test_next_enables_auto_merge_for_open_pr(tmp_path):
    """Enables auto-merge when PR is open."""
    mock_wave = MagicMock()
    mock_wave.name = "test"
    mock_wave.id = "wave-123"

    with patch("loopflow.lf.ops.next.get_wave_by_worktree", return_value=mock_wave):
        with patch("loopflow.lf.ops.next.find_main_repo", return_value=tmp_path):
            with patch("loopflow.lf.ops.next.get_default_branch", return_value="main"):
                with patch("loopflow.lf.ops.next.get_current_branch", return_value="feature"):
                    with patch("loopflow.lf.ops.next._rebase_onto_main", return_value=True):
                        with patch("loopflow.lf.ops.next._get_pr_number", return_value=42):
                            with patch("loopflow.lf.ops.next._get_pr_state", return_value="OPEN"):
                                with patch("loopflow.lf.ops.next._enable_auto_merge") as mock_merge:
                                    mock_merge.return_value = True
                                    with patch(
                                        "loopflow.lf.ops.next.generate_branch_name",
                                        return_value="new-branch",
                                    ):
                                        with patch("loopflow.lf.ops.next.git_create_branch"):
                                            with patch("subprocess.run"):
                                                with patch("loopflow.lf.ops.next.update_wave_worktree_branch"):
                                                    next_iteration(tmp_path)

    mock_merge.assert_called_once_with(tmp_path, 42)
