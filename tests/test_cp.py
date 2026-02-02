"""Tests for lf ops cp command."""

from unittest.mock import patch

import pytest
from typer.testing import CliRunner

from loopflow.lf.cli import app


@pytest.fixture
def temp_repo(tmp_path):
    """Create a minimal repo for testing."""
    (tmp_path / ".git").mkdir()
    (tmp_path / "README.md").write_text("# Test Project\n")
    (tmp_path / ".lf").mkdir()
    return tmp_path


def test_cp_copies_docs_by_default(temp_repo, monkeypatch):
    """cp with no args copies docs and shows token breakdown."""
    monkeypatch.chdir(temp_repo)
    runner = CliRunner()

    with patch("loopflow.lf.ops.cp.copy_to_clipboard") as mock_copy:
        result = runner.invoke(app, ["ops", "cp"])

    assert result.exit_code == 0
    assert "Copied to clipboard" in result.output
    copied_text = mock_copy.call_args[0][0]
    assert "# Test Project" in copied_text


def test_cp_includes_context_files(temp_repo, monkeypatch):
    """cp includes specified files as positional args."""
    (temp_repo / "main.py").write_text("print('hello')\n")
    monkeypatch.chdir(temp_repo)
    runner = CliRunner()

    with patch("loopflow.lf.ops.cp.copy_to_clipboard") as mock_copy:
        result = runner.invoke(app, ["ops", "cp", "main.py"])

    assert result.exit_code == 0
    copied_text = mock_copy.call_args[0][0]
    assert "print('hello')" in copied_text


def test_cp_no_lfdocs_excludes_documentation(temp_repo, monkeypatch):
    """cp --no-lfdocs excludes repo .md files."""
    monkeypatch.chdir(temp_repo)
    runner = CliRunner()

    with patch("loopflow.lf.ops.cp.copy_to_clipboard") as mock_copy:
        result = runner.invoke(app, ["ops", "cp", "--no-lfdocs"])

    assert result.exit_code == 0
    copied_text = mock_copy.call_args[0][0]
    assert "# Test Project" not in copied_text


def test_cp_exclude_patterns(temp_repo, monkeypatch):
    """cp -e excludes matching files."""
    (temp_repo / "main.py").write_text("print('main')\n")
    (temp_repo / "test.py").write_text("print('test')\n")
    monkeypatch.chdir(temp_repo)
    runner = CliRunner()

    with patch("loopflow.lf.ops.cp.copy_to_clipboard") as mock_copy:
        result = runner.invoke(app, ["ops", "cp", "*.py", "-e", "test.py"])

    assert result.exit_code == 0
    copied_text = mock_copy.call_args[0][0]
    assert "print('main')" in copied_text
    assert "print('test')" not in copied_text


def test_cp_works_outside_git_repo(tmp_path, monkeypatch):
    """cp works outside a git repo (uses cwd as fallback)."""
    (tmp_path / "README.md").write_text("# No Git Here\n")
    monkeypatch.chdir(tmp_path)
    runner = CliRunner()

    with patch("loopflow.lf.ops.cp.copy_to_clipboard") as mock_copy:
        result = runner.invoke(app, ["ops", "cp"])

    assert result.exit_code == 0
    assert "Copied to clipboard" in result.output
    copied_text = mock_copy.call_args[0][0]
    assert "# No Git Here" in copied_text


def test_cp_positional_args_as_context(temp_repo, monkeypatch):
    """cp accepts positional arguments as context files."""
    (temp_repo / "src").mkdir()
    (temp_repo / "src" / "main.py").write_text("print('main')\n")
    (temp_repo / "tests").mkdir()
    (temp_repo / "tests" / "test_main.py").write_text("print('test')\n")
    monkeypatch.chdir(temp_repo)
    runner = CliRunner()

    with patch("loopflow.lf.ops.cp.copy_to_clipboard") as mock_copy:
        result = runner.invoke(app, ["ops", "cp", "src", "tests"])

    assert result.exit_code == 0
    copied_text = mock_copy.call_args[0][0]
    assert "print('main')" in copied_text
    assert "print('test')" in copied_text


def test_cp_outputs_files_without_instructional_prompts(temp_repo, monkeypatch):
    """cp outputs file content with lf:file tags but no instructional text."""
    (temp_repo / "main.py").write_text("print('hello')\n")
    monkeypatch.chdir(temp_repo)
    runner = CliRunner()

    with patch("loopflow.lf.ops.cp.copy_to_clipboard") as mock_copy:
        result = runner.invoke(app, ["ops", "cp", "main.py"])

    assert result.exit_code == 0
    copied_text = mock_copy.call_args[0][0]
    # Should have file content with lf:file tags
    assert "print('hello')" in copied_text
    assert "<lf:files>" in copied_text
    assert '<lf:file path="main.py">' in copied_text
    # Should NOT have instructional wrapper content
    assert "<lf:loopflow>" not in copied_text
    assert "<lf:docs>" not in copied_text
    assert "Repository documentation" not in copied_text
    assert "Follow STYLE carefully" not in copied_text
