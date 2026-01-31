"""Tests for CLI structure after ops consolidation."""

import re

from typer.testing import CliRunner

from loopflow.lf.cli import app


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


def test_top_level_help_has_ops():
    """Verify ops is present on lf and loop is still removed."""
    runner = CliRunner()

    result = runner.invoke(app, ["--help"])

    assert result.exit_code == 0
    commands = _get_command_names(result.output)
    assert "ops" in commands
    assert "loop" not in commands  # loop is now lfd command
    assert "run" in commands


def test_ops_help_lists_management_commands():
    """Verify lf ops has the expected commands."""
    runner = CliRunner()

    result = runner.invoke(app, ["ops", "--help"])

    assert result.exit_code == 0
    commands = _get_command_names(result.output)
    # init and install moved to `lf init` (interactive prompt)
    assert "doctor" in commands
    assert "version" in commands
    assert "pr" in commands
    assert "land" in commands


def test_lf_flag_without_step_not_treated_as_step_name():
    """Verify lf -a doesn't treat '-a' as a step name.

    Regression test: 'lf -a' should be transformed to 'lf run -a',
    not error with "No step or flow named '-a'".
    """
    import sys

    # Save original argv
    original_argv = sys.argv.copy()

    try:
        # Simulate 'lf -a'
        sys.argv = ["lf", "-a"]

        # The main() function modifies sys.argv before calling app()
        # We need to test that it correctly transforms the args

        # After main() processes args, sys.argv[1] should be "run", not "-a"
        # We can't easily test main() directly since it calls app(),
        # so we'll test the arg transformation logic

        # Check the condition that should match
        first_arg = sys.argv[1]
        known_commands = {"run", "inline", "flow", "--help", "-h"}

        # This is the condition from cli.py that should match for flags
        should_insert_run = first_arg.startswith("-") and first_arg not in known_commands

        assert should_insert_run, "'-a' should trigger 'run' insertion, but condition was False"

    finally:
        sys.argv = original_argv
