"""Tests for agent database operations."""

from datetime import datetime
from pathlib import Path
import uuid

from loopflow.maestro.agent import (
    AgentLoopSpec,
    AgentStatus,
    OuterLoopConfig,
    OuterLoopMode,
    RegisteredAgent,
)
from loopflow.maestro.db import (
    delete_agent,
    increment_agent_iteration,
    load_agent,
    load_agent_by_name,
    load_agents,
    save_agent,
    update_agent_status,
)


def _make_agent(name: str = "test-agent") -> RegisteredAgent:
    """Create a test agent."""
    return RegisteredAgent(
        id=str(uuid.uuid4()),
        spec=AgentLoopSpec(
            name=name,
            prompt_path=Path("prompt.md"),
            pipeline=["implement", "review"],
            context=["src/"],
            outer_loop=OuterLoopConfig(mode=OuterLoopMode.PR_CHAIN),
        ),
        status=AgentStatus.IDLE,
    )


def test_save_and_load_agent(tmp_path):
    """save_agent and load_agents round-trip."""
    db_path = tmp_path / "test.db"
    agent = _make_agent()

    save_agent(db_path, agent)
    agents = load_agents(db_path)

    assert len(agents) == 1
    loaded = agents[0]
    assert loaded.id == agent.id
    assert loaded.spec.name == agent.spec.name
    assert loaded.spec.pipeline == agent.spec.pipeline
    assert loaded.spec.context == agent.spec.context
    assert loaded.spec.outer_loop.mode == agent.spec.outer_loop.mode
    assert loaded.status == AgentStatus.IDLE


def test_load_agent_by_id(tmp_path):
    """load_agent returns agent by ID."""
    db_path = tmp_path / "test.db"
    agent = _make_agent()

    save_agent(db_path, agent)
    loaded = load_agent(db_path, agent.id)

    assert loaded is not None
    assert loaded.id == agent.id
    assert loaded.spec.name == agent.spec.name


def test_load_agent_not_found(tmp_path):
    """load_agent returns None for missing ID."""
    db_path = tmp_path / "test.db"
    agent = _make_agent()

    save_agent(db_path, agent)
    loaded = load_agent(db_path, "nonexistent")

    assert loaded is None


def test_load_agent_by_name(tmp_path):
    """load_agent_by_name returns agent by spec name."""
    db_path = tmp_path / "test.db"

    agent1 = _make_agent("agent-one")
    agent2 = _make_agent("agent-two")

    save_agent(db_path, agent1)
    save_agent(db_path, agent2)

    loaded = load_agent_by_name(db_path, "agent-two")
    assert loaded is not None
    assert loaded.id == agent2.id
    assert loaded.spec.name == "agent-two"


def test_load_agent_by_name_not_found(tmp_path):
    """load_agent_by_name returns None for missing name."""
    db_path = tmp_path / "test.db"
    agent = _make_agent("exists")

    save_agent(db_path, agent)
    loaded = load_agent_by_name(db_path, "missing")

    assert loaded is None


def test_update_agent_status(tmp_path):
    """update_agent_status modifies status."""
    db_path = tmp_path / "test.db"
    agent = _make_agent()

    save_agent(db_path, agent)
    updated = update_agent_status(db_path, agent.id, AgentStatus.RUNNING)

    assert updated is True

    loaded = load_agent(db_path, agent.id)
    assert loaded.status == AgentStatus.RUNNING
    assert loaded.last_run_at is not None


def test_update_agent_status_with_runtime_fields(tmp_path):
    """update_agent_status can set pid, worktree, branch."""
    db_path = tmp_path / "test.db"
    agent = _make_agent()

    save_agent(db_path, agent)
    update_agent_status(
        db_path,
        agent.id,
        AgentStatus.RUNNING,
        pid=12345,
        worktree=Path("/project/worktree"),
        branch="agent/test/1",
    )

    loaded = load_agent(db_path, agent.id)
    assert loaded.pid == 12345
    assert loaded.current_worktree == Path("/project/worktree")
    assert loaded.current_branch == "agent/test/1"


def test_increment_agent_iteration(tmp_path):
    """increment_agent_iteration bumps counter."""
    db_path = tmp_path / "test.db"
    agent = _make_agent()
    agent.iteration = 5

    save_agent(db_path, agent)
    increment_agent_iteration(db_path, agent.id)

    loaded = load_agent(db_path, agent.id)
    assert loaded.iteration == 6


def test_delete_agent(tmp_path):
    """delete_agent removes agent."""
    db_path = tmp_path / "test.db"
    agent = _make_agent()

    save_agent(db_path, agent)
    deleted = delete_agent(db_path, agent.id)

    assert deleted is True

    agents = load_agents(db_path)
    assert len(agents) == 0


def test_delete_agent_not_found(tmp_path):
    """delete_agent returns False for missing ID."""
    db_path = tmp_path / "test.db"
    agent = _make_agent()

    save_agent(db_path, agent)
    deleted = delete_agent(db_path, "nonexistent")

    assert deleted is False


def test_save_agent_replaces_existing(tmp_path):
    """save_agent replaces agent with same ID."""
    db_path = tmp_path / "test.db"
    agent = _make_agent()

    save_agent(db_path, agent)

    # Modify and save again
    agent.spec.pipeline = ["design"]
    agent.iteration = 10
    save_agent(db_path, agent)

    agents = load_agents(db_path)
    assert len(agents) == 1
    assert agents[0].spec.pipeline == ["design"]
    assert agents[0].iteration == 10


def test_load_agents_empty_db(tmp_path):
    """load_agents returns empty list for empty db."""
    db_path = tmp_path / "test.db"

    agents = load_agents(db_path)

    assert agents == []


def test_load_agents_nonexistent_db(tmp_path):
    """load_agents returns empty list for nonexistent db."""
    db_path = tmp_path / "nonexistent.db"

    agents = load_agents(db_path)

    assert agents == []
