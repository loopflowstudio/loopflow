"""Tests for lfd daemon."""

import tempfile
from datetime import datetime
from pathlib import Path

from loopflow.lfd.models import (
    AgentSpec,
    AgentRun,
    AgentStatus,
    MergeStrategy,
    Session,
    SessionStatus,
    TriggerKind,
    TriggerSpec,
)
from loopflow.lfd.protocol import Request, Response, Event, success, error
from loopflow.lfd.db import save_run, load_agent_runs, get_latest_run, save_session, load_sessions


def test_trigger_spec_serialization():
    spec = TriggerSpec(kind=TriggerKind.INTERVAL, interval_seconds=300)
    data = spec.to_dict()
    restored = TriggerSpec.from_dict(data)
    assert restored.kind == TriggerKind.INTERVAL
    assert restored.interval_seconds == 300


def test_trigger_spec_with_cron():
    spec = TriggerSpec(kind=TriggerKind.CRON, cron="0 9 * * *", grace_minutes=120)
    data = spec.to_dict()
    restored = TriggerSpec.from_dict(data)
    assert restored.kind == TriggerKind.CRON
    assert restored.cron == "0 9 * * *"
    assert restored.grace_minutes == 120


def test_agent_spec_serialization():
    spec = AgentSpec(
        name="test-agent",
        repo=Path("/tmp/repo"),
        pipeline="ship",
        trigger=TriggerSpec(kind=TriggerKind.MAIN_CHANGED),
        context=["src/"],
        prompt="Test prompt",
    )
    data = spec.to_dict()
    restored = AgentSpec.from_dict(data)
    assert restored.name == "test-agent"
    assert restored.pipeline == "ship"
    assert restored.trigger.kind == TriggerKind.MAIN_CHANGED


def test_agent_spec_with_emoji_and_goal():
    spec = AgentSpec(
        name="security-bot",
        repo=Path("/tmp/repo"),
        pipeline="ship",
        emoji="🔒",
        goal=Path(".lf/goals/security.md"),
        merge_strategy=MergeStrategy.AUTO,
    )
    data = spec.to_dict()
    restored = AgentSpec.from_dict(data)
    assert restored.emoji == "🔒"
    assert restored.goal == Path(".lf/goals/security.md")
    assert restored.merge_strategy == MergeStrategy.AUTO


def test_agent_run_serialization():
    run = AgentRun(
        id="run-1",
        agent_name="test-agent",
        status=AgentStatus.RUNNING,
        started_at=datetime(2024, 1, 1, 12, 0, 0),
        pid=1234,
        iteration=5,
    )
    data = run.to_dict()
    restored = AgentRun.from_dict(data)
    assert restored.id == "run-1"
    assert restored.status == AgentStatus.RUNNING
    assert restored.iteration == 5


def test_agent_run_with_emoji():
    run = AgentRun(
        id="run-1",
        agent_name="security-bot",
        status=AgentStatus.RUNNING,
        started_at=datetime(2024, 1, 1, 12, 0, 0),
        emoji="🔒",
        iteration=1,
    )
    data = run.to_dict()
    assert data["emoji"] == "🔒"
    restored = AgentRun.from_dict(data)
    assert restored.emoji == "🔒"


def test_session_serialization():
    session = Session(
        id="sess-1",
        task="implement",
        repo="/tmp/repo",
        worktree="/tmp/repo.feature",
        status=SessionStatus.RUNNING,
        started_at=datetime(2024, 1, 1, 12, 0, 0),
        model="claude-code",
        run_mode="auto",
    )
    data = session.to_dict()
    restored = Session.from_dict(data)
    assert restored.task == "implement"
    assert restored.status == SessionStatus.RUNNING


def test_protocol_request_parse():
    line = '{"method": "agents.list", "params": {"name": "test"}}'
    request = Request.parse(line)
    assert request.method == "agents.list"
    assert request.params == {"name": "test"}


def test_protocol_response_serialize():
    resp = success({"agents": 5}, id="req-1")
    serialized = resp.serialize()
    assert '"ok": true' in serialized
    assert '"result":' in serialized


def test_protocol_error_response():
    resp = error("Not found")
    serialized = resp.serialize()
    assert '"ok": false' in serialized
    assert '"error": "Not found"' in serialized


def test_protocol_event_serialize():
    event = Event("agent.started", {"name": "test", "pid": 1234})
    serialized = event.serialize()
    assert '"event": "agent.started"' in serialized


def test_db_save_and_load_run():
    with tempfile.TemporaryDirectory() as tmpdir:
        db_path = Path(tmpdir) / "test.db"
        run = AgentRun(
            id="run-1",
            agent_name="test-agent",
            status=AgentStatus.RUNNING,
            started_at=datetime.now(),
            pid=1234,
            iteration=1,
        )
        save_run(run, db_path)

        runs = load_agent_runs(db_path=db_path)
        assert len(runs) == 1
        assert runs[0].agent_name == "test-agent"


def test_db_get_latest_run():
    with tempfile.TemporaryDirectory() as tmpdir:
        db_path = Path(tmpdir) / "test.db"

        # Save two runs
        run1 = AgentRun(
            id="run-1",
            agent_name="test-agent",
            status=AgentStatus.STOPPED,
            started_at=datetime(2024, 1, 1, 12, 0, 0),
            iteration=1,
        )
        run2 = AgentRun(
            id="run-2",
            agent_name="test-agent",
            status=AgentStatus.RUNNING,
            started_at=datetime(2024, 1, 2, 12, 0, 0),
            iteration=2,
        )
        save_run(run1, db_path)
        save_run(run2, db_path)

        latest = get_latest_run("test-agent", db_path)
        assert latest is not None
        assert latest.id == "run-2"
        assert latest.iteration == 2


def test_db_save_and_load_session():
    with tempfile.TemporaryDirectory() as tmpdir:
        db_path = Path(tmpdir) / "test.db"
        session = Session(
            id="sess-1",
            task="implement",
            repo="/tmp/repo",
            worktree="/tmp/repo.feature",
            status=SessionStatus.RUNNING,
            started_at=datetime.now(),
        )
        save_session(session, db_path)

        sessions = load_sessions(db_path=db_path)
        assert len(sessions) == 1
        assert sessions[0].task == "implement"
