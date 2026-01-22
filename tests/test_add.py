"""Tests for lfops add command."""

import pytest
from typer.testing import CliRunner

from loopflow.lfops.commands import app


@pytest.fixture
def temp_repo(tmp_path):
    """Create a minimal repo for testing."""
    (tmp_path / ".git").mkdir()
    return tmp_path


def test_add_creates_prompt_file(temp_repo, monkeypatch):
    """lfops add creates .claude/commands/<name>.md."""
    monkeypatch.chdir(temp_repo)
    runner = CliRunner()

    result = runner.invoke(app, ["add", "review"])

    assert result.exit_code == 0
    assert "Created .claude/commands/review.md" in result.output

    target = temp_repo / ".claude" / "commands" / "review.md"
    assert target.exists()
    content = target.read_text()
    assert "produces:" in content
    assert "Review step." in content
    assert "{args}" in content


def test_add_errors_if_file_exists(temp_repo, monkeypatch):
    """lfops add errors when file already exists."""
    commands_dir = temp_repo / ".claude" / "commands"
    commands_dir.mkdir(parents=True)
    (commands_dir / "review.md").write_text("existing")
    monkeypatch.chdir(temp_repo)
    runner = CliRunner()

    result = runner.invoke(app, ["add", "review"])

    assert result.exit_code == 1
    assert "already exists" in result.output


def test_add_force_overwrites(temp_repo, monkeypatch):
    """lfops add -f overwrites existing file."""
    commands_dir = temp_repo / ".claude" / "commands"
    commands_dir.mkdir(parents=True)
    (commands_dir / "review.md").write_text("existing")
    monkeypatch.chdir(temp_repo)
    runner = CliRunner()

    result = runner.invoke(app, ["add", "review", "-f"])

    assert result.exit_code == 0
    content = (commands_dir / "review.md").read_text()
    assert "Review step." in content


def test_add_requires_git_repo(tmp_path, monkeypatch):
    """lfops add requires a git repository."""
    monkeypatch.chdir(tmp_path)
    runner = CliRunner()

    result = runner.invoke(app, ["add", "review"])

    assert result.exit_code == 1
    assert "git repository" in result.output
