"""Tests for agent loop data structures and serialization."""

from datetime import datetime
from pathlib import Path

from loopflow.maestro.agent import (
    AgentLoopSpec,
    AgentStatus,
    OuterLoopConfig,
    OuterLoopMode,
    RegisteredAgent,
)


def test_outer_loop_config_serialization():
    """OuterLoopConfig round-trips through dict."""
    config = OuterLoopConfig(mode=OuterLoopMode.PR_CHAIN)

    data = config.to_dict()
    assert data["mode"] == "pr-chain"

    restored = OuterLoopConfig.from_dict(data)
    assert restored.mode == OuterLoopMode.PR_CHAIN


def test_outer_loop_config_land_commits():
    """OuterLoopConfig handles land-commits mode."""
    config = OuterLoopConfig(mode=OuterLoopMode.LAND_COMMITS)

    data = config.to_dict()
    assert data["mode"] == "land-commits"

    restored = OuterLoopConfig.from_dict(data)
    assert restored.mode == OuterLoopMode.LAND_COMMITS


def test_agent_loop_spec_serialization():
    """AgentLoopSpec round-trips through dict."""
    spec = AgentLoopSpec(
        name="test-agent",
        prompt_path=Path("prompts/test.md"),
        pipeline=["design", "implement", "review"],
        context=["src/", "tests/"],
        outer_loop=OuterLoopConfig(mode=OuterLoopMode.PR_CHAIN),
    )

    data = spec.to_dict()
    assert data["name"] == "test-agent"
    assert data["prompt_path"] == "prompts/test.md"
    assert data["pipeline"] == ["design", "implement", "review"]
    assert data["context"] == ["src/", "tests/"]
    assert data["outer_loop"]["mode"] == "pr-chain"

    restored = AgentLoopSpec.from_dict(data)
    assert restored.name == spec.name
    assert restored.prompt_path == spec.prompt_path
    assert restored.pipeline == spec.pipeline
    assert restored.context == spec.context
    assert restored.outer_loop.mode == spec.outer_loop.mode


def test_agent_loop_spec_defaults():
    """AgentLoopSpec has sensible defaults."""
    spec = AgentLoopSpec(
        name="minimal",
        prompt_path=Path("prompt.md"),
        pipeline=["implement"],
    )

    assert spec.context == []
    assert spec.outer_loop.mode == OuterLoopMode.LAND_COMMITS


def test_agent_loop_spec_empty_context():
    """AgentLoopSpec handles missing context in deserialization."""
    data = {
        "name": "test",
        "prompt_path": "prompt.md",
        "pipeline": ["implement"],
        "outer_loop": {"mode": "land-commits"},
    }

    spec = AgentLoopSpec.from_dict(data)
    assert spec.context == []


def test_registered_agent_serialization():
    """RegisteredAgent round-trips through dict."""
    agent = RegisteredAgent(
        id="test-id-123",
        spec=AgentLoopSpec(
            name="test-agent",
            prompt_path=Path("prompt.md"),
            pipeline=["implement"],
        ),
        status=AgentStatus.RUNNING,
        last_run_at=datetime(2025, 1, 1, 12, 0, 0),
        current_worktree=Path("/project/worktree"),
        current_branch="agent/test-agent/1",
        iteration=5,
        pid=12345,
    )

    data = agent.to_dict()
    assert data["id"] == "test-id-123"
    assert data["status"] == "running"
    assert data["iteration"] == 5
    assert data["pid"] == 12345
    assert data["current_branch"] == "agent/test-agent/1"

    restored = RegisteredAgent.from_dict(data)
    assert restored.id == agent.id
    assert restored.status == agent.status
    assert restored.last_run_at == agent.last_run_at
    assert restored.current_worktree == agent.current_worktree
    assert restored.current_branch == agent.current_branch
    assert restored.iteration == agent.iteration
    assert restored.pid == agent.pid


def test_registered_agent_defaults():
    """RegisteredAgent has sensible defaults."""
    agent = RegisteredAgent(
        id="test-id",
        spec=AgentLoopSpec(
            name="test",
            prompt_path=Path("prompt.md"),
            pipeline=["implement"],
        ),
    )

    assert agent.status == AgentStatus.IDLE
    assert agent.last_run_at is None
    assert agent.current_worktree is None
    assert agent.current_branch is None
    assert agent.iteration == 0
    assert agent.pid is None


def test_registered_agent_optional_fields():
    """RegisteredAgent handles None fields in deserialization."""
    data = {
        "id": "test-id",
        "spec": {
            "name": "test",
            "prompt_path": "prompt.md",
            "pipeline": ["implement"],
            "outer_loop": {"mode": "land-commits"},
        },
        "status": "idle",
    }

    agent = RegisteredAgent.from_dict(data)
    assert agent.last_run_at is None
    assert agent.current_worktree is None
    assert agent.current_branch is None
    assert agent.iteration == 0
    assert agent.pid is None


def test_agent_status_values():
    """AgentStatus has expected values."""
    assert AgentStatus.IDLE.value == "idle"
    assert AgentStatus.RUNNING.value == "running"
    assert AgentStatus.ERROR.value == "error"


def test_outer_loop_mode_values():
    """OuterLoopMode has expected values."""
    assert OuterLoopMode.PR_CHAIN.value == "pr-chain"
    assert OuterLoopMode.LAND_COMMITS.value == "land-commits"
