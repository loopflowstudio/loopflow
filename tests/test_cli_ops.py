"""Tests for CLI structure after ops removal."""

import re

from typer.testing import CliRunner

from loopflow.lf import app
from loopflow.lfops import get_app


def _strip_ansi(text: str) -> str:
    """Remove ANSI escape codes from text."""
    return re.sub(r"\x1b\[[0-9;]*m", "", text)


def _get_command_names(output: str) -> list[str]:
    """Extract command names from Typer help output.

    Handles both plain text "Commands:" and Rich-formatted "╭─ Commands ─..."
    """
    output = _strip_ansi(output)
    lines = output.splitlines()

    # Find the commands section (handles Rich box formatting)
    start = None
    for i, line in enumerate(lines):
        if "Commands" in line:
            start = i
            break

    if start is None:
        return []

    commands = []
    for line in lines[start + 1 :]:
        # Stop at section end (Rich box border or empty line)
        if line.startswith("╰") or (not line.strip() and commands):
            break
        # Extract command name from lines like "│ run        description │"
        stripped = line.strip().lstrip("│").strip()
        if stripped and not stripped.startswith("─"):
            parts = stripped.split()
            if parts:
                commands.append(parts[0])
    return commands


def test_top_level_help_no_ops():
    """Verify ops subcommand was removed from lf."""
    runner = CliRunner()

    result = runner.invoke(app, ["--help"])

    assert result.exit_code == 0
    commands = _get_command_names(result.output)
    assert "ops" not in commands
    assert "run" in commands
    assert "loop" in commands


def test_lfops_help_lists_management_commands():
    """Verify lfops has the expected commands."""
    runner = CliRunner()
    lfops_app = get_app()

    result = runner.invoke(lfops_app, ["--help"])

    assert result.exit_code == 0
    commands = _get_command_names(result.output)
    assert "init" in commands
    assert "install" in commands
    assert "doctor" in commands
    assert "version" in commands
    assert "pr" in commands
    assert "land" in commands
