"""Tests for agent naming conventions."""

from pathlib import Path

from loopflow.lfd.models import AgentSpec, TriggerSpec
from loopflow.lfd.naming import (
    agent_branch_name,
    agent_worktree_name,
    agent_worktree_path,
    agent_pr_title,
    parse_agent_branch,
)


def _make_agent(name: str, emoji: str = "") -> AgentSpec:
    return AgentSpec(
        name=name,
        repo=Path("/tmp/repo"),
        pipeline="ship",
        emoji=emoji,
    )


def test_agent_branch_name_with_emoji():
    agent = _make_agent("security-bot", emoji="🔒")
    branch = agent_branch_name(agent, 1)
    assert branch == "🔒/security-bot/001"


def test_agent_branch_name_without_emoji():
    agent = _make_agent("refactor-bot")
    branch = agent_branch_name(agent, 42)
    assert branch == "agent/refactor-bot/042"


def test_agent_worktree_name_with_emoji():
    agent = _make_agent("security-bot", emoji="🔒")
    name = agent_worktree_name("loopflow", agent, 1)
    assert name == "loopflow.🔒-security-bot-001"


def test_agent_worktree_name_without_emoji():
    agent = _make_agent("refactor-bot")
    name = agent_worktree_name("loopflow", agent, 5)
    assert name == "loopflow.agent-refactor-bot-005"


def test_agent_worktree_path():
    agent = _make_agent("security-bot", emoji="🔒")
    agent.repo = Path("/Users/jack/src/loopflow")
    path = agent_worktree_path(agent.repo, agent, 1)
    assert path == Path("/Users/jack/src/loopflow.🔒-security-bot-001")


def test_agent_pr_title_with_emoji():
    agent = _make_agent("security-bot", emoji="🔒")
    title = agent_pr_title(agent, "Add input validation")
    assert title == "🔒 Add input validation"


def test_agent_pr_title_without_emoji():
    agent = _make_agent("refactor-bot")
    title = agent_pr_title(agent, "Simplify auth module")
    assert title == "Simplify auth module"


def test_parse_agent_branch_with_emoji():
    result = parse_agent_branch("🔒/security-bot/001")
    assert result == ("🔒", "security-bot", 1)


def test_parse_agent_branch_without_emoji():
    result = parse_agent_branch("agent/refactor-bot/042")
    # "agent" prefix means no emoji (empty string)
    assert result == ("", "refactor-bot", 42)


def test_parse_agent_branch_invalid():
    assert parse_agent_branch("main") is None
    assert parse_agent_branch("feature/auth") is None
    assert parse_agent_branch("🔒/bot/abc") is None
