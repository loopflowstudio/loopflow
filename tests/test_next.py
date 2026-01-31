"""Tests for next command."""

from pathlib import Path
from unittest.mock import MagicMock, patch

import pytest

from loopflow.lf.ops.next import (
    _enable_auto_merge,
    _get_pr_number,
    _get_pr_state,
    next_worktree,
)


@pytest.fixture
def mock_repo(tmp_path):
    """Create a mock repo directory."""
    return tmp_path / "repo"


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
            # First call edits PR, second enables auto-merge
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


def test_next_fails_on_main_branch():
    """Cannot run next from main branch."""
    with patch("loopflow.lf.ops.next.find_main_repo", return_value=Path("/repo")):
        with patch("loopflow.lf.ops.next.get_default_branch", return_value="main"):
            result = next_worktree(Path("/repo"), "main", block=False, open_terminal=False)
    assert result is None


def test_next_fails_without_pr():
    """Fails when no PR exists, branch not merged, and create_pr not set."""
    with patch("loopflow.lf.ops.next.find_main_repo", return_value=Path("/repo")):
        with patch("loopflow.lf.ops.next.get_default_branch", return_value="main"):
            with patch("loopflow.lf.ops.next._rebase_onto_main", return_value=True):
                with patch("loopflow.lf.ops.next._get_pr_number", return_value=None):
                    with patch("loopflow.lf.ops.next._fetch_main"):
                        with patch("loopflow.lf.ops.next._is_branch_merged", return_value=False):
                            result = next_worktree(
                                Path("/repo"),
                                "feature-branch",
                                block=False,
                                open_terminal=False,
                                create_pr=False,
                            )
    assert result is None


def test_next_creates_worktree_with_suffix(tmp_path):
    """Creates new worktree with magical-musical suffix."""
    repo = tmp_path / "repo"
    repo.mkdir()
    preserved_path = tmp_path / "repo.jack.auth.20260123_1112.suffix"
    new_worktree = tmp_path / "repo.jack.auth.20260123_1112"

    with patch("loopflow.lf.ops.next.find_main_repo", return_value=repo):
        with patch("loopflow.lf.ops.next.get_default_branch", return_value="main"):
            with patch("loopflow.lf.ops.next._rebase_onto_main", return_value=True):
                with patch("loopflow.lf.ops.next._get_pr_number", return_value=42):
                    with patch("loopflow.lf.ops.next._get_pr_state", return_value="OPEN"):
                        with patch("loopflow.lf.ops.next._enable_auto_merge", return_value=True):
                            with patch("loopflow.lf.ops.next._wait_for_merge", return_value=False):
                                with patch(
                                    "loopflow.lf.ops.next.generate_next_branch",
                                    return_value="jack.auth.20260123_1112-aurora-melody",
                                ):
                                    with patch(
                                        "loopflow.lf.ops.next._preserve_worktree",
                                        return_value=preserved_path,
                                    ):
                                        with patch(
                                            "loopflow.lf.ops.next._create_fresh_worktree",
                                            return_value=new_worktree,
                                        ):
                                            with patch("subprocess.run") as mock_run:
                                                # git rev-parse HEAD for stacking
                                                mock_run.return_value = MagicMock(
                                                    returncode=0, stdout="abc123\n"
                                                )
                                                with patch(
                                                    "loopflow.lf.ops.next.get_wave_by_worktree",
                                                    return_value=None,
                                                ):
                                                    with patch(
                                                        "loopflow.lf.ops.next.write_directive"
                                                    ):
                                                        result = next_worktree(
                                                            repo,
                                                            "jack.auth.20260123_1112",
                                                            block=False,
                                                            open_terminal=False,
                                                        )

    assert result is not None
    assert result == new_worktree


def test_next_starts_fresh_when_already_merged(tmp_path):
    """When branch is already merged, starts fresh from origin/main."""
    repo = tmp_path / "repo"
    repo.mkdir()
    preserved_path = tmp_path / "repo.wave.wisp-forte"
    new_worktree = tmp_path / "repo.wave"

    with patch("loopflow.lf.ops.next.find_main_repo", return_value=repo):
        with patch("loopflow.lf.ops.next.get_default_branch", return_value="main"):
            with patch("loopflow.lf.ops.next._rebase_onto_main", return_value=True):
                with patch("loopflow.lf.ops.next._get_pr_number", return_value=42):
                    with patch("loopflow.lf.ops.next._get_pr_state", return_value="MERGED"):
                        with patch("loopflow.lf.ops.next._fetch_main"):
                            with patch(
                                "loopflow.lf.ops.next.parse_branch_base",
                                return_value="wave",
                            ):
                                with patch(
                                    "loopflow.lf.ops.next._preserve_worktree",
                                    return_value=preserved_path,
                                ):
                                    with patch(
                                        "loopflow.lf.ops.next.generate_next_branch",
                                        return_value="wave.20260130_1200-aurora-melody",
                                    ):
                                        with patch(
                                            "loopflow.lf.ops.next._create_fresh_worktree",
                                            return_value=new_worktree,
                                        ):
                                            with patch(
                                                "loopflow.lf.ops.next.get_wave_by_worktree",
                                                return_value=None,
                                            ):
                                                with patch("loopflow.lf.ops.next.write_directive"):
                                                    result = next_worktree(
                                                        repo,
                                                        "wave.20260129_1100.wisp-forte",
                                                        block=False,
                                                        open_terminal=False,
                                                    )

    assert result is not None
    assert result == new_worktree
