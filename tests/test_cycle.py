"""Tests for cycle command."""

from pathlib import Path
from unittest.mock import MagicMock, patch

import pytest

from loopflow.lfops.cycle import (
    _enable_auto_merge,
    _get_pr_number,
    _get_pr_state,
    cycle,
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


def test_cycle_fails_on_main_branch():
    """Cannot cycle from main branch."""
    with patch("loopflow.lfops.cycle.find_main_repo", return_value=Path("/repo")):
        with patch("loopflow.lfops.cycle.get_default_branch", return_value="main"):
            result = cycle(Path("/repo"), "main", wait=False, open_terminal=False)
    assert result is None


def test_cycle_fails_without_pr():
    """Fails when no PR exists and create_pr not set."""
    with patch("loopflow.lfops.cycle.find_main_repo", return_value=Path("/repo")):
        with patch("loopflow.lfops.cycle.get_default_branch", return_value="main"):
            with patch("loopflow.lfops.cycle._get_pr_number", return_value=None):
                result = cycle(
                    Path("/repo"),
                    "feature-branch",
                    wait=False,
                    open_terminal=False,
                    create_pr=False,
                )
    assert result is None


def test_cycle_creates_worktree_with_suffix(tmp_path):
    """Creates new worktree with magical-musical suffix."""
    repo = tmp_path / "repo"
    repo.mkdir()

    with patch("loopflow.lfops.cycle.find_main_repo", return_value=repo):
        with patch("loopflow.lfops.cycle.get_default_branch", return_value="main"):
            with patch("loopflow.lfops.cycle._get_pr_number", return_value=42):
                with patch("loopflow.lfops.cycle._enable_auto_merge", return_value=True):
                    with patch("loopflow.lfops.cycle._wait_for_merge", return_value=False):
                        with patch(
                            "loopflow.lfops.cycle.generate_cycle_branch",
                            return_value="jack.auth.20260123_1112-aurora-melody",
                        ):
                            with patch(
                                "loopflow.lfops.cycle.parse_branch_for_cycle",
                                return_value="jack.auth.20260123_1112",
                            ):
                                with patch("subprocess.run") as mock_run:
                                    # git worktree add succeeds
                                    mock_run.return_value = MagicMock(returncode=0)
                                    with patch("loopflow.lfops.cycle.write_directive"):
                                        result = cycle(
                                            repo,
                                            "jack.auth.20260123_1112",
                                            wait=False,
                                            open_terminal=False,
                                        )

    assert result is not None
    # Worktree path should be sibling to repo
    assert result.parent == repo.parent
