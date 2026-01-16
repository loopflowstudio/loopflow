"""Tests for loopflow.capture module."""

from pathlib import Path
from unittest.mock import patch

import pytest

from loopflow.capture import (
    WindowInfo,
    find_window,
    generate_screenshot_path,
)


@pytest.fixture
def mock_windows():
    """Sample windows for testing matching logic."""
    return [
        WindowInfo(window_id=1, app_name="Maestro", title="Main Window", bounds={}),
        WindowInfo(window_id=2, app_name="Terminal", title="~/src/loopflow", bounds={}),
        WindowInfo(window_id=3, app_name="Cursor", title="loopflow - STYLE.md", bounds={}),
        WindowInfo(window_id=4, app_name="Safari", title="GitHub - anthropics/claude-code", bounds={}),
        WindowInfo(window_id=5, app_name="Maestro", title="Settings", bounds={}),
    ]


class TestFindWindow:
    """Tests for window name matching."""

    def test_exact_match_app_name(self, mock_windows):
        """Exact app name match returns that window."""
        with patch("loopflow.capture.list_windows", return_value=mock_windows):
            result = find_window("Maestro")
            assert result is not None
            assert result.app_name == "Maestro"

    def test_exact_match_case_insensitive(self, mock_windows):
        """Matching is case-insensitive."""
        with patch("loopflow.capture.list_windows", return_value=mock_windows):
            result = find_window("maestro")
            assert result is not None
            assert result.app_name == "Maestro"

    def test_exact_match_title(self, mock_windows):
        """Exact title match works."""
        with patch("loopflow.capture.list_windows", return_value=mock_windows):
            result = find_window("Settings")
            assert result is not None
            assert result.title == "Settings"

    def test_prefix_match(self, mock_windows):
        """Prefix matches work."""
        with patch("loopflow.capture.list_windows", return_value=mock_windows):
            result = find_window("Term")
            assert result is not None
            assert result.app_name == "Terminal"

    def test_substring_match(self, mock_windows):
        """Substring matches work."""
        with patch("loopflow.capture.list_windows", return_value=mock_windows):
            result = find_window("loopflow")
            assert result is not None
            # Could match Terminal or Cursor - both have loopflow in title

    def test_exact_beats_prefix(self, mock_windows):
        """Exact match is preferred over prefix match."""
        windows = [
            WindowInfo(window_id=1, app_name="Terminal", title="", bounds={}),
            WindowInfo(window_id=2, app_name="Term", title="", bounds={}),
        ]
        with patch("loopflow.capture.list_windows", return_value=windows):
            result = find_window("Term")
            assert result is not None
            assert result.window_id == 2  # Exact match

    def test_prefix_beats_substring(self, mock_windows):
        """Prefix match is preferred over substring match."""
        windows = [
            WindowInfo(window_id=1, app_name="MyTerminal", title="", bounds={}),
            WindowInfo(window_id=2, app_name="Terminal", title="", bounds={}),
        ]
        with patch("loopflow.capture.list_windows", return_value=windows):
            result = find_window("Terminal")
            assert result is not None
            assert result.window_id == 2  # Exact > prefix, but prefix > substring

    def test_no_match_returns_none(self, mock_windows):
        """No match returns None."""
        with patch("loopflow.capture.list_windows", return_value=mock_windows):
            result = find_window("NonexistentApp")
            assert result is None

    def test_empty_window_list(self):
        """Empty window list returns None."""
        with patch("loopflow.capture.list_windows", return_value=[]):
            result = find_window("Anything")
            assert result is None


class TestGenerateScreenshotPath:
    """Tests for screenshot path generation."""

    def test_named_screenshot(self, tmp_path):
        """Named screenshot uses provided name."""
        path = generate_screenshot_path("main-view", tmp_path)
        assert path == tmp_path / ".design" / "screenshots" / "main-view.png"

    def test_timestamped_screenshot(self, tmp_path):
        """No name generates timestamped filename."""
        path = generate_screenshot_path(None, tmp_path)
        assert path.parent == tmp_path / ".design" / "screenshots"
        assert path.name.startswith("capture-")
        assert path.suffix == ".png"

    def test_path_structure(self, tmp_path):
        """Screenshots go to .design/screenshots/."""
        path = generate_screenshot_path("test", tmp_path)
        assert ".design" in path.parts
        assert "screenshots" in path.parts
