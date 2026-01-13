"""Tests for ops subcommand wiring."""

from typer.testing import CliRunner

from loopflow.cli import app


def _get_command_names(output: str) -> list[str]:
    lines = output.splitlines()
    try:
        start = lines.index("Commands:")
    except ValueError:
        return []

    commands = []
    for line in lines[start + 1 :]:
        if not line.strip():
            break
        if line.startswith("  "):
            commands.append(line.split()[0])
    return commands


def test_top_level_help_lists_ops_only():
    runner = CliRunner()

    result = runner.invoke(app, ["--help"])

    assert result.exit_code == 0
    commands = _get_command_names(result.output)
    assert "ops" in commands
    assert "pr" not in commands
    assert "meta" not in commands
    assert "maestro" not in commands
    assert "status" not in commands
    assert "compare" not in commands
    assert "land" not in commands
    assert "stop" not in commands
    assert "prune" not in commands


def test_ops_help_lists_management_commands():
    runner = CliRunner()

    result = runner.invoke(app, ["ops", "--help"])

    assert result.exit_code == 0
    commands = _get_command_names(result.output)
    assert "pr" in commands
    assert "meta" in commands
    assert "maestro" in commands
    assert "status" in commands
    assert "stop" in commands
    assert "prune" in commands
    assert "compare" in commands
    assert "land" in commands
