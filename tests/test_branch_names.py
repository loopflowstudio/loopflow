"""Tests for branch name schema formatting."""

import os
from datetime import datetime
from unittest.mock import MagicMock, patch

import pytest

from loopflow.lf.branch_names import _get_git_username, _sanitize_for_branch, format_branch_name
from loopflow.lf.config import BranchNameConfig


def _make_config(schema: str) -> BranchNameConfig:
    """Create BranchNameConfig using the YAML alias."""
    return BranchNameConfig.model_validate({"schema": schema})


def test_format_branch_name_returns_short_name_when_no_config():
    """No config means no transformation."""
    result = format_branch_name("my-feature", None)
    assert result == "my-feature"


def test_format_branch_name_returns_short_name_with_default_schema():
    """Default schema {name} returns short name unchanged."""
    config = _make_config("{name}")
    result = format_branch_name("my-feature", config)
    assert result == "my-feature"


def test_format_branch_name_expands_user_placeholder():
    """User placeholder is expanded from git config."""
    config = _make_config("{user}.{name}")
    with patch("loopflow.lf.branch_names._get_git_username", return_value="alice"):
        result = format_branch_name("my-feature", config)
    assert result == "alice.my-feature"


def test_format_branch_name_expands_ts_placeholder():
    """Timestamp placeholder uses YYYYMMDD_HHMM format."""
    config = _make_config("{name}.{ts}")
    fixed_now = MagicMock()
    fixed_now.strftime.side_effect = lambda fmt: datetime(2026, 1, 20, 14, 30).strftime(fmt)
    with patch("loopflow.lf.branch_names.datetime") as mock_dt:
        mock_dt.now.return_value = fixed_now
        result = format_branch_name("my-feature", config)
    assert result == "my-feature.20260120_1430"


def test_format_branch_name_expands_date_placeholder():
    """Date placeholder uses YYYYMMDD format."""
    config = _make_config("{name}.{date}")
    fixed_now = MagicMock()
    fixed_now.strftime.side_effect = lambda fmt: datetime(2026, 1, 20, 14, 30).strftime(fmt)
    with patch("loopflow.lf.branch_names.datetime") as mock_dt:
        mock_dt.now.return_value = fixed_now
        result = format_branch_name("my-feature", config)
    assert result == "my-feature.20260120"


def test_format_branch_name_expands_all_placeholders():
    """All placeholders can be combined."""
    config = _make_config("{user}.{name}.{ts}")
    fixed_now = MagicMock()
    fixed_now.strftime.side_effect = lambda fmt: datetime(2026, 3, 15, 9, 5).strftime(fmt)
    with patch("loopflow.lf.branch_names._get_git_username", return_value="bob"):
        with patch("loopflow.lf.branch_names.datetime") as mock_dt:
            mock_dt.now.return_value = fixed_now
            result = format_branch_name("fix-bug", config)
    assert result == "bob.fix-bug.20260315_0905"


def test_sanitize_for_branch_replaces_spaces():
    """Spaces become hyphens."""
    assert _sanitize_for_branch("Jack Heart") == "jack-heart"


def test_sanitize_for_branch_removes_special_chars():
    """Special characters like @ are replaced."""
    assert _sanitize_for_branch("user@domain") == "user-domain"


def test_sanitize_for_branch_preserves_dots():
    """Dots are valid in git branch names and preserved."""
    assert _sanitize_for_branch("v1.2.3") == "v1.2.3"


def test_sanitize_for_branch_collapses_multiple_hyphens():
    """Multiple consecutive hyphens become one."""
    assert _sanitize_for_branch("a   b") == "a-b"


def test_sanitize_for_branch_strips_leading_trailing():
    """Leading and trailing hyphens are removed."""
    assert _sanitize_for_branch(" test ") == "test"


def test_get_git_username_from_git_config():
    """Username comes from git config user.name."""
    with patch("subprocess.run") as mock_run:
        mock_run.return_value = MagicMock(returncode=0, stdout="Alice Smith\n")
        result = _get_git_username()
    assert result == "alice-smith"


def test_get_git_username_falls_back_to_env():
    """Falls back to $USER when git config fails."""
    with patch("subprocess.run") as mock_run:
        mock_run.return_value = MagicMock(returncode=1, stdout="")
        with patch.dict(os.environ, {"USER": "bob"}):
            result = _get_git_username()
    assert result == "bob"
