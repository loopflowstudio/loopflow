"""Tests for lfd daemon."""

import tempfile
from datetime import datetime
from pathlib import Path

from loopflow.lfd.models import (
    AgentSpec,
    AgentRun,
    AgentStatus,
    MergeMode,
    Session,
    SessionStatus,
    TriggerKind,
    TriggerSpec,
)
from loopflow.lfd.protocol import Request, Response, Event, success, error
from loopflow.lfd.db import (
    save_run,
    load_agent_runs,
    get_latest_run,
    save_session,
    load_sessions,
    load_sessions_for_worktree,
    load_sessions_for_repo,
    update_session_status,
)


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
        merge_mode=MergeMode.AUTO,
    )
    data = spec.to_dict()
    restored = AgentSpec.from_dict(data)
    assert restored.emoji == "🔒"
    assert restored.goal == Path(".lf/goals/security.md")
    assert restored.merge_mode == MergeMode.AUTO


def test_agent_spec_with_personal_main():
    spec = AgentSpec(
        name="my-agent",
        repo=Path("/tmp/repo"),
        pipeline="ship",
        merge_mode=MergeMode.SILENT,
        personal_main="my-agent-main",
    )
    data = spec.to_dict()
    restored = AgentSpec.from_dict(data)
    assert restored.personal_main == "my-agent-main"
    assert restored.merge_mode == MergeMode.SILENT


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


def test_db_load_sessions_for_worktree():
    with tempfile.TemporaryDirectory() as tmpdir:
        db_path = Path(tmpdir) / "test.db"

        session1 = Session(
            id="sess-1",
            task="implement",
            repo="/tmp/repo",
            worktree="/tmp/repo.feature-a",
            status=SessionStatus.COMPLETED,
            started_at=datetime(2024, 1, 1, 12, 0, 0),
        )
        session2 = Session(
            id="sess-2",
            task="review",
            repo="/tmp/repo",
            worktree="/tmp/repo.feature-a",
            status=SessionStatus.COMPLETED,
            started_at=datetime(2024, 1, 2, 12, 0, 0),
        )
        session3 = Session(
            id="sess-3",
            task="implement",
            repo="/tmp/repo",
            worktree="/tmp/repo.feature-b",
            status=SessionStatus.COMPLETED,
            started_at=datetime.now(),
        )

        save_session(session1, db_path)
        save_session(session2, db_path)
        save_session(session3, db_path)

        sessions = load_sessions_for_worktree("/tmp/repo.feature-a", db_path=db_path)
        assert len(sessions) == 2
        # Should be ordered by started_at DESC
        assert sessions[0].id == "sess-2"
        assert sessions[1].id == "sess-1"


def test_db_load_sessions_for_repo():
    with tempfile.TemporaryDirectory() as tmpdir:
        db_path = Path(tmpdir) / "test.db"

        session1 = Session(
            id="sess-1",
            task="implement",
            repo="/tmp/repo-a",
            worktree="/tmp/repo-a.feature",
            status=SessionStatus.COMPLETED,
            started_at=datetime.now(),
        )
        session2 = Session(
            id="sess-2",
            task="review",
            repo="/tmp/repo-b",
            worktree="/tmp/repo-b.feature",
            status=SessionStatus.COMPLETED,
            started_at=datetime.now(),
        )

        save_session(session1, db_path)
        save_session(session2, db_path)

        sessions = load_sessions_for_repo("/tmp/repo-a", db_path=db_path)
        assert len(sessions) == 1
        assert sessions[0].id == "sess-1"


def test_db_update_session_status():
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

        updated = update_session_status("sess-1", SessionStatus.COMPLETED, db_path)
        assert updated is True

        sessions = load_sessions_for_worktree("/tmp/repo.feature", db_path=db_path)
        assert len(sessions) == 1
        assert sessions[0].status == SessionStatus.COMPLETED
        assert sessions[0].ended_at is not None


def test_db_update_session_status_nonexistent():
    with tempfile.TemporaryDirectory() as tmpdir:
        db_path = Path(tmpdir) / "test.db"
        # Initialize DB by saving and then checking update
        session = Session(
            id="sess-1",
            task="implement",
            repo="/tmp/repo",
            worktree="/tmp/repo.feature",
            status=SessionStatus.RUNNING,
            started_at=datetime.now(),
        )
        save_session(session, db_path)

        updated = update_session_status("nonexistent", SessionStatus.COMPLETED, db_path)
        assert updated is False


# Server handler tests


def test_server_handle_output_line_broadcasts_event():
    """output.line handler broadcasts event to subscribers."""
    import asyncio
    from loopflow.lfd.server import Server

    async def run_test():
        with tempfile.TemporaryDirectory() as tmpdir:
            socket_path = Path(tmpdir) / "test.sock"
            server = Server(socket_path)

            # Create mock writers to track broadcasts
            broadcast_events = []

            async def mock_broadcast(event):
                broadcast_events.append(event)

            server._broadcast = mock_broadcast

            # Call the handler
            params = {"session_id": "test-session-123", "text": "→ Read: foo.py"}
            response = await server._handle_output_line(params)

            # Check response
            assert response.ok is True
            assert response.result == {}

            # Check event was broadcast
            assert len(broadcast_events) == 1
            event = broadcast_events[0]
            assert event.event == "output.line"
            assert event.data["session_id"] == "test-session-123"
            assert event.data["text"] == "→ Read: foo.py"
            assert "timestamp" in event.data

    asyncio.run(run_test())


def test_server_handle_output_line_missing_session_id():
    """output.line handler returns error for missing session_id."""
    import asyncio
    from loopflow.lfd.server import Server

    async def run_test():
        with tempfile.TemporaryDirectory() as tmpdir:
            socket_path = Path(tmpdir) / "test.sock"
            server = Server(socket_path)

            params = {"text": "→ Read: foo.py"}
            response = await server._handle_output_line(params)

            assert response.ok is False
            assert "session_id" in response.error

    asyncio.run(run_test())


def test_server_handle_output_line_missing_text():
    """output.line handler returns error for missing text."""
    import asyncio
    from loopflow.lfd.server import Server

    async def run_test():
        with tempfile.TemporaryDirectory() as tmpdir:
            socket_path = Path(tmpdir) / "test.sock"
            server = Server(socket_path)

            params = {"session_id": "test-session-123"}
            response = await server._handle_output_line(params)

            assert response.ok is False
            assert "text" in response.error

    asyncio.run(run_test())


def test_server_handle_output_line_allows_empty_text():
    """output.line handler accepts empty string for text (allows blank lines)."""
    import asyncio
    from loopflow.lfd.server import Server

    async def run_test():
        with tempfile.TemporaryDirectory() as tmpdir:
            socket_path = Path(tmpdir) / "test.sock"
            server = Server(socket_path)

            broadcast_events = []
            server._broadcast = lambda e: broadcast_events.append(e) or asyncio.sleep(0)

            # Empty string should be allowed (it's a blank line in output)
            params = {"session_id": "test-session-123", "text": ""}
            response = await server._handle_output_line(params)

            assert response.ok is True
            assert len(broadcast_events) == 1
            assert broadcast_events[0].data["text"] == ""

    asyncio.run(run_test())
