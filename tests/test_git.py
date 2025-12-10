"""Tests for loopflow.git module."""

import subprocess
from pathlib import Path
from unittest.mock import MagicMock, patch

from loopflow.git import has_upstream, push


def test_has_upstream_returns_true_when_tracking():
    """has_upstream returns True when branch tracks remote."""
    with patch("subprocess.run") as mock_run:
        mock_run.return_value = MagicMock(returncode=0)
        result = has_upstream(Path("/fake/repo"))
        assert result is True


def test_has_upstream_returns_false_when_not_tracking():
    """has_upstream returns False when branch doesn't track remote."""
    with patch("subprocess.run") as mock_run:
        mock_run.return_value = MagicMock(returncode=128)
        result = has_upstream(Path("/fake/repo"))
        assert result is False


def test_push_returns_true_on_success():
    """push returns True when git push succeeds."""
    with patch("subprocess.run") as mock_run:
        mock_run.return_value = MagicMock(returncode=0)
        result = push(Path("/fake/repo"))
        assert result is True


def test_push_returns_false_on_failure():
    """push returns False when git push fails."""
    with patch("subprocess.run") as mock_run:
        mock_run.return_value = MagicMock(returncode=1)
        result = push(Path("/fake/repo"))
        assert result is False
