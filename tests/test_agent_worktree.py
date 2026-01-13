"""Tests for agent worktree management."""

from pathlib import Path
from unittest.mock import MagicMock, patch

import pytest

from loopflow.maestro.markdown import AgentFile, parse_agent_file
from loopflow.maestro.worktree import (
    _agent_branch_name,
    ensure_agent_worktree,
    get_agent_worktree_path,
    remove_agent_worktree,
)


def _make_agent(name: str = "test-agent", repo: Path = Path("/repo"), fresh: bool = False) -> AgentFile:
    return AgentFile(
        name=name,
        path=Path.home() / ".lf" / "agents" / f"{name}.md",
        repo=repo,
        pipeline=["implement", "polish"],
        trigger="manual",
        context=[],
        prompt="Test prompt",
        fresh=fresh,
    )


def test_agent_branch_name():
    """Branch name is prefixed with agent-."""
    agent = _make_agent(name="docs-bot")
    assert _agent_branch_name(agent) == "agent-docs-bot"


def test_ensure_agent_worktree_creates_new(tmp_path):
    """Creates worktree when none exists."""
    agent = _make_agent(repo=tmp_path)
    expected_path = tmp_path.parent / f"{tmp_path.name}.agent-{agent.name}"

    with patch("loopflow.maestro.worktree.list_all", return_value=[]):
        with patch("loopflow.maestro.worktree.create", return_value=expected_path) as mock_create:
            result = ensure_agent_worktree(agent)

            mock_create.assert_called_once_with(tmp_path, "agent-test-agent", base="main")
            assert result == expected_path


def test_ensure_agent_worktree_syncs_existing(tmp_path):
    """Syncs existing worktree to main."""
    agent = _make_agent(repo=tmp_path)
    expected_path = tmp_path.parent / f"{tmp_path.name}.agent-{agent.name}"
    expected_path.mkdir(parents=True)

    mock_wt = MagicMock()
    mock_wt.branch = "agent-test-agent"

    with patch("loopflow.maestro.worktree.list_all", return_value=[mock_wt]):
        with patch("loopflow.maestro.worktree._sync_to_main", return_value=True):
            with patch("loopflow.maestro.worktree.get_path", return_value=expected_path):
                result = ensure_agent_worktree(agent)

                assert result == expected_path


def test_ensure_agent_worktree_fresh_recreates(tmp_path):
    """fresh=True removes and recreates worktree."""
    agent = _make_agent(repo=tmp_path, fresh=True)
    expected_path = tmp_path.parent / f"{tmp_path.name}.agent-{agent.name}"

    mock_wt = MagicMock()
    mock_wt.branch = "agent-test-agent"

    with patch("loopflow.maestro.worktree.list_all", return_value=[mock_wt]):
        with patch("loopflow.maestro.worktree.remove") as mock_remove:
            with patch("loopflow.maestro.worktree.create", return_value=expected_path) as mock_create:
                result = ensure_agent_worktree(agent)

                # Should remove existing worktree first
                mock_remove.assert_called_once_with(tmp_path, "agent-test-agent")
                # Then create fresh
                mock_create.assert_called_once_with(tmp_path, "agent-test-agent", base="main")
                assert result == expected_path


def test_get_agent_worktree_path_returns_path_when_exists(tmp_path):
    """Returns worktree path when it exists."""
    agent = _make_agent(repo=tmp_path)
    expected_path = tmp_path.parent / f"{tmp_path.name}.agent-{agent.name}"

    mock_wt = MagicMock()
    mock_wt.branch = "agent-test-agent"
    mock_wt.path = expected_path

    with patch("loopflow.maestro.worktree.list_all", return_value=[mock_wt]):
        result = get_agent_worktree_path(agent)
        assert result == expected_path


def test_get_agent_worktree_path_returns_none_when_missing(tmp_path):
    """Returns None when worktree doesn't exist."""
    agent = _make_agent(repo=tmp_path)

    with patch("loopflow.maestro.worktree.list_all", return_value=[]):
        result = get_agent_worktree_path(agent)
        assert result is None


def test_remove_agent_worktree_calls_remove(tmp_path):
    """remove_agent_worktree calls worktree remove."""
    agent = _make_agent(repo=tmp_path)

    with patch("loopflow.maestro.worktree.remove", return_value=True) as mock_remove:
        result = remove_agent_worktree(agent)

        mock_remove.assert_called_once_with(tmp_path, "agent-test-agent")
        assert result is True


def test_parse_agent_file_with_fresh(tmp_path):
    """parse_agent_file handles fresh: true."""
    agent_file = tmp_path / "test.md"
    agent_file.write_text("""---
repo: /repo
pipeline: [implement, polish]
trigger: main-changed
fresh: true
---

Test prompt.
""")

    agent = parse_agent_file(agent_file)
    assert agent is not None
    assert agent.fresh is True


def test_parse_agent_file_fresh_defaults_false(tmp_path):
    """parse_agent_file defaults fresh to False."""
    agent_file = tmp_path / "test.md"
    agent_file.write_text("""---
repo: /repo
pipeline: [implement]
trigger: manual
---

Test prompt.
""")

    agent = parse_agent_file(agent_file)
    assert agent is not None
    assert agent.fresh is False
