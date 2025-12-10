"""Tests for loopflow.launcher module."""

from loopflow.launcher import check_claude_available


def test_check_claude_available_returns_bool():
    """check_claude_available returns a boolean."""
    result = check_claude_available()
    assert isinstance(result, bool)
