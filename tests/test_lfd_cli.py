"""Tests for lfd CLI commands.

Tests agent CRUD operations and resolution without requiring the daemon.
"""

import pytest
from typer.testing import CliRunner

import loopflow.lfd.db as db_module
from loopflow.lfd.cli import app

runner = CliRunner()


@pytest.fixture
def test_env(tmp_path, monkeypatch):
    """Set up test environment with temp database and mock repo."""
    # Create mock repo
    repo_path = tmp_path / "repo"
    repo_path.mkdir()
    (repo_path / ".git").mkdir()

    # Set up temp database path
    db_path = tmp_path / "test.db"

    # Patch DB_PATH at the module level
    monkeypatch.setattr(db_module, "DB_PATH", db_path)

    # Patch get_wt_from_cwd to return our mock repo
    monkeypatch.setattr("loopflow.lfd.cli.get_wt_from_cwd", lambda: repo_path)

    return {"repo": repo_path, "db": db_path}


class TestCreateCommand:
    def test_create_with_name(self, test_env):
        """lfd create <name> creates agent with given name."""
        result = runner.invoke(app, ["create", "test-agent"])

        assert result.exit_code == 0
        assert "Created" in result.output
        assert "test-agent" in result.output

    def test_create_generates_name(self, test_env):
        """lfd create (no name) generates a name."""
        result = runner.invoke(app, ["create"])

        assert result.exit_code == 0
        assert "Created" in result.output

    def test_create_duplicate_name_noop(self, test_env):
        """lfd create <name> with existing name is a no-op."""
        # Create first agent
        runner.invoke(app, ["create", "duplicate-agent"])
        # Try to create again
        result = runner.invoke(app, ["create", "duplicate-agent"])

        assert result.exit_code == 0
        assert "already exists" in result.output


class TestAreaCommand:
    def test_area_sets_agent_area(self, test_env):
        """lfd area <name> <path> sets the agent's area."""
        # Create agent first
        runner.invoke(app, ["create", "area-test"])
        # Set area
        result = runner.invoke(app, ["area", "area-test", "src/"])

        assert result.exit_code == 0
        assert "Set area: src/" in result.output

    def test_area_not_found(self, test_env):
        """lfd area <nonexistent> errors."""
        result = runner.invoke(app, ["area", "nonexistent", "src/"])

        assert result.exit_code == 1
        assert "not found" in result.output


class TestGoalCommand:
    def test_goal_sets_agent_goal(self, test_env):
        """lfd goal <name> <text> sets the agent's goal."""
        # Create agent first
        runner.invoke(app, ["create", "goal-test"])
        # Set goal
        result = runner.invoke(app, ["goal", "goal-test", "fix lint errors"])

        assert result.exit_code == 0
        assert "Set goal: fix lint errors" in result.output


class TestShowCommand:
    def test_show_displays_agent(self, test_env):
        """lfd show <name> displays agent details."""
        # Create and configure agent
        runner.invoke(app, ["create", "show-test"])
        runner.invoke(app, ["area", "show-test", "src/"])
        # Show agent
        result = runner.invoke(app, ["show", "show-test"])

        assert result.exit_code == 0
        assert "show-test" in result.output
        assert "src/" in result.output

    def test_show_not_found(self, test_env):
        """lfd show <nonexistent> errors."""
        result = runner.invoke(app, ["show", "nonexistent"])

        assert result.exit_code == 1
        assert "not found" in result.output


class TestListCommand:
    def test_list_shows_agents(self, test_env):
        """lfd list shows all agents."""
        # Create agents
        runner.invoke(app, ["create", "agent-one"])
        runner.invoke(app, ["create", "agent-two"])
        # List
        result = runner.invoke(app, ["list"])

        assert result.exit_code == 0
        assert "agent-one" in result.output
        assert "agent-two" in result.output

    def test_list_empty(self, test_env):
        """lfd list with no agents shows message."""
        result = runner.invoke(app, ["list"])

        assert result.exit_code == 0
        assert "No agents configured" in result.output


class TestRmCommand:
    def test_rm_deletes_agent(self, test_env):
        """lfd rm <id> -f deletes the agent."""
        # Create agent
        runner.invoke(app, ["create", "rm-test"])
        # Get the agent ID by listing
        list_result = runner.invoke(app, ["list"])
        # Extract short ID from output (last column)
        lines = list_result.output.strip().split("\n")
        agent_line = [line for line in lines if "rm-test" in line][0]
        short_id = agent_line.split()[-1]

        # Delete with force
        result = runner.invoke(app, ["rm", short_id, "-f"])

        assert result.exit_code == 0
        assert "Deleted" in result.output

    def test_rm_not_found(self, test_env):
        """lfd rm <nonexistent> errors."""
        result = runner.invoke(app, ["rm", "nonexistent", "-f"])

        assert result.exit_code == 1
        assert "not found" in result.output


class TestAgentResolution:
    def test_resolve_by_name(self, test_env):
        """Agent commands resolve by name."""
        runner.invoke(app, ["create", "resolve-name"])
        result = runner.invoke(app, ["area", "resolve-name", "lib/"])

        assert result.exit_code == 0
        assert "Set area" in result.output

    def test_resolve_by_short_id(self, test_env):
        """Agent commands resolve by short ID."""
        runner.invoke(app, ["create", "resolve-id"])
        # Get short ID
        list_result = runner.invoke(app, ["list"])
        lines = list_result.output.strip().split("\n")
        agent_line = [line for line in lines if "resolve-id" in line][0]
        short_id = agent_line.split()[-1]

        # Use short ID to set area
        result = runner.invoke(app, ["area", short_id, "tests/"])

        assert result.exit_code == 0
        assert "Set area" in result.output


class TestValidation:
    def test_loop_without_area_fails(self, test_env):
        """lfd loop without area configured errors."""
        # Create agent without area
        runner.invoke(app, ["create", "no-area"])
        # Try to loop
        result = runner.invoke(app, ["loop", "no-area"])

        assert result.exit_code == 1
        assert "no area configured" in result.output

    def test_run_without_area_fails(self, test_env):
        """lfd run without area configured errors."""
        # Create agent without area
        runner.invoke(app, ["create", "run-no-area"])
        # Try to run
        result = runner.invoke(app, ["run", "run-no-area"])

        assert result.exit_code == 1
        assert "no area configured" in result.output
