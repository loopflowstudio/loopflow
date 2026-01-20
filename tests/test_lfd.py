"""Tests for lfd daemon."""

import tempfile
from datetime import datetime
from pathlib import Path

from loopflow.lfd.models import (
    AgentSpec,
    AgentRun,
    AgentStatus,
    Loop,
    LoopRun,
    LoopStatus,
    LoopType,
    MergeMode,
    Session,
    SessionStatus,
    TriggerKind,
    TriggerSpec,
)
from loopflow.lfd.protocol import Request, Response, Event, success, error
from loopflow.lfd.db import (
    delete_loop,
    get_latest_loop_run,
    get_loop,
    get_loop_by_goal_repo,
    get_loop_runs,
    get_latest_run,
    list_loops,
    load_agent_runs,
    save_loop,
    save_loop_run,
    save_run,
    save_session,
    load_sessions,
    load_sessions_for_worktree,
    load_sessions_for_repo,
    update_loop_iteration,
    update_loop_pid,
    update_loop_run_pr,
    update_loop_run_status,
    update_loop_run_step,
    update_loop_status,
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
        merge_mode=MergeMode.PR,
        personal_main="my-agent-main",
    )
    data = spec.to_dict()
    restored = AgentSpec.from_dict(data)
    assert restored.personal_main == "my-agent-main"
    assert restored.merge_mode == MergeMode.PR


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


# Loop model tests


def test_loop_model_defaults():
    """Loop model has correct defaults."""
    loop = Loop(
        id="loop-1",
        type=LoopType.LOOP,
        goal="test-coverage",
        repo=Path("/tmp/repo"),
        personal_main="test-coverage-main",
    )
    assert loop.status == LoopStatus.IDLE
    assert loop.iteration == 0
    assert loop.pr_limit == 5
    assert loop.merge_mode == MergeMode.AUTO
    assert loop.project_file is None
    assert loop.pathset is None
    assert loop.cron is None
    assert loop.area is None
    assert loop.pid is None


def test_loop_model_short_id():
    """Loop.short_id() returns first 7 chars."""
    loop = Loop(
        id="abcdef1234567890",
        type=LoopType.LOOP,
        goal="test",
        repo=Path("/tmp/repo"),
        personal_main="test-main",
    )
    assert loop.short_id() == "abcdef1"


def test_loop_run_model():
    """LoopRun model stores iteration data."""
    run = LoopRun(
        id="run-1",
        loop_id="loop-1",
        iteration=3,
        status=LoopStatus.RUNNING,
        started_at=datetime.now(),
        worktree="/tmp/repo.wt",
        current_step="implement",
    )
    assert run.iteration == 3
    assert run.status == LoopStatus.RUNNING
    assert run.current_step == "implement"
    assert run.ended_at is None
    assert run.error is None
    assert run.pr_url is None


# Loop database tests


def test_db_save_and_get_loop():
    """Save and retrieve a loop."""
    with tempfile.TemporaryDirectory() as tmpdir:
        db_path = Path(tmpdir) / "test.db"
        loop = Loop(
            id="loop-123",
            type=LoopType.LOOP,
            goal="test-coverage",
            repo=Path("/tmp/repo"),
            personal_main="test-coverage-main",
            status=LoopStatus.IDLE,
            iteration=0,
            pr_limit=5,
        )
        save_loop(loop, db_path)

        loaded = get_loop("loop-123", db_path)
        assert loaded is not None
        assert loaded.id == "loop-123"
        assert loaded.goal == "test-coverage"
        assert loaded.type == LoopType.LOOP
        assert loaded.personal_main == "test-coverage-main"


def test_db_get_loop_short_id():
    """Get loop by short ID prefix."""
    with tempfile.TemporaryDirectory() as tmpdir:
        db_path = Path(tmpdir) / "test.db"
        loop = Loop(
            id="abcdef1234567890",
            type=LoopType.LOOP,
            goal="test",
            repo=Path("/tmp/repo"),
            personal_main="test-main",
        )
        save_loop(loop, db_path)

        # Should find by prefix
        loaded = get_loop("abcdef1", db_path)
        assert loaded is not None
        assert loaded.id == "abcdef1234567890"


def test_db_get_loop_by_goal_repo():
    """Get loop by type, goal, and repo."""
    with tempfile.TemporaryDirectory() as tmpdir:
        db_path = Path(tmpdir) / "test.db"
        loop = Loop(
            id="loop-1",
            type=LoopType.FLOW,
            goal="api-cleanup",
            repo=Path("/tmp/repo"),
            personal_main="api-cleanup-main",
        )
        save_loop(loop, db_path)

        loaded = get_loop_by_goal_repo(LoopType.FLOW, "api-cleanup", Path("/tmp/repo"), db_path)
        assert loaded is not None
        assert loaded.id == "loop-1"

        # Different type should not match
        not_found = get_loop_by_goal_repo(LoopType.LOOP, "api-cleanup", Path("/tmp/repo"), db_path)
        assert not_found is None


def test_db_list_loops():
    """List all loops."""
    with tempfile.TemporaryDirectory() as tmpdir:
        db_path = Path(tmpdir) / "test.db"

        loop1 = Loop(
            id="loop-1",
            type=LoopType.LOOP,
            goal="goal-a",
            repo=Path("/tmp/repo-a"),
            personal_main="goal-a-main",
        )
        loop2 = Loop(
            id="loop-2",
            type=LoopType.SUBSCRIBE,
            goal="goal-b",
            repo=Path("/tmp/repo-b"),
            personal_main="goal-b-main",
        )
        save_loop(loop1, db_path)
        save_loop(loop2, db_path)

        loops = list_loops(db_path=db_path)
        assert len(loops) == 2

        # Filter by repo
        loops = list_loops(repo=Path("/tmp/repo-a"), db_path=db_path)
        assert len(loops) == 1
        assert loops[0].goal == "goal-a"


def test_db_update_loop_status():
    """Update loop status."""
    with tempfile.TemporaryDirectory() as tmpdir:
        db_path = Path(tmpdir) / "test.db"
        loop = Loop(
            id="loop-1",
            type=LoopType.LOOP,
            goal="test",
            repo=Path("/tmp/repo"),
            personal_main="test-main",
            status=LoopStatus.IDLE,
        )
        save_loop(loop, db_path)

        updated = update_loop_status("loop-1", LoopStatus.RUNNING, db_path)
        assert updated is True

        loaded = get_loop("loop-1", db_path)
        assert loaded.status == LoopStatus.RUNNING


def test_db_update_loop_iteration():
    """Update loop iteration count."""
    with tempfile.TemporaryDirectory() as tmpdir:
        db_path = Path(tmpdir) / "test.db"
        loop = Loop(
            id="loop-1",
            type=LoopType.LOOP,
            goal="test",
            repo=Path("/tmp/repo"),
            personal_main="test-main",
            iteration=0,
        )
        save_loop(loop, db_path)

        updated = update_loop_iteration("loop-1", 5, db_path)
        assert updated is True

        loaded = get_loop("loop-1", db_path)
        assert loaded.iteration == 5


def test_db_delete_loop():
    """Delete loop and its runs."""
    with tempfile.TemporaryDirectory() as tmpdir:
        db_path = Path(tmpdir) / "test.db"
        loop = Loop(
            id="loop-1",
            type=LoopType.LOOP,
            goal="test",
            repo=Path("/tmp/repo"),
            personal_main="test-main",
        )
        save_loop(loop, db_path)

        # Add a run
        run = LoopRun(
            id="run-1",
            loop_id="loop-1",
            iteration=1,
            status=LoopStatus.RUNNING,
            started_at=datetime.now(),
        )
        save_loop_run(run, db_path)

        # Delete loop (should also delete runs)
        deleted = delete_loop("loop-1", db_path)
        assert deleted is True

        assert get_loop("loop-1", db_path) is None
        assert get_loop_runs("loop-1", db_path=db_path) == []


# Loop run database tests


def test_db_save_and_get_loop_runs():
    """Save and retrieve loop runs."""
    with tempfile.TemporaryDirectory() as tmpdir:
        db_path = Path(tmpdir) / "test.db"

        # Create parent loop first
        loop = Loop(
            id="loop-1",
            type=LoopType.LOOP,
            goal="test",
            repo=Path("/tmp/repo"),
            personal_main="test-main",
        )
        save_loop(loop, db_path)

        run1 = LoopRun(
            id="run-1",
            loop_id="loop-1",
            iteration=1,
            status=LoopStatus.IDLE,
            started_at=datetime(2024, 1, 1, 12, 0, 0),
            pr_url="https://github.com/user/repo/pull/1",
        )
        run2 = LoopRun(
            id="run-2",
            loop_id="loop-1",
            iteration=2,
            status=LoopStatus.RUNNING,
            started_at=datetime(2024, 1, 2, 12, 0, 0),
        )
        save_loop_run(run1, db_path)
        save_loop_run(run2, db_path)

        runs = get_loop_runs("loop-1", db_path=db_path)
        assert len(runs) == 2
        # Ordered by started_at DESC
        assert runs[0].id == "run-2"
        assert runs[1].id == "run-1"


def test_db_get_latest_loop_run():
    """Get most recent run for a loop."""
    with tempfile.TemporaryDirectory() as tmpdir:
        db_path = Path(tmpdir) / "test.db"

        loop = Loop(
            id="loop-1",
            type=LoopType.LOOP,
            goal="test",
            repo=Path("/tmp/repo"),
            personal_main="test-main",
        )
        save_loop(loop, db_path)

        run1 = LoopRun(
            id="run-1",
            loop_id="loop-1",
            iteration=1,
            status=LoopStatus.IDLE,
            started_at=datetime(2024, 1, 1, 12, 0, 0),
        )
        run2 = LoopRun(
            id="run-2",
            loop_id="loop-1",
            iteration=2,
            status=LoopStatus.RUNNING,
            started_at=datetime(2024, 1, 2, 12, 0, 0),
        )
        save_loop_run(run1, db_path)
        save_loop_run(run2, db_path)

        latest = get_latest_loop_run("loop-1", db_path)
        assert latest is not None
        assert latest.id == "run-2"


def test_db_update_loop_run_status():
    """Update loop run status."""
    with tempfile.TemporaryDirectory() as tmpdir:
        db_path = Path(tmpdir) / "test.db"

        loop = Loop(
            id="loop-1",
            type=LoopType.LOOP,
            goal="test",
            repo=Path("/tmp/repo"),
            personal_main="test-main",
        )
        save_loop(loop, db_path)

        run = LoopRun(
            id="run-1",
            loop_id="loop-1",
            iteration=1,
            status=LoopStatus.RUNNING,
            started_at=datetime.now(),
        )
        save_loop_run(run, db_path)

        updated = update_loop_run_status("run-1", LoopStatus.IDLE, db_path=db_path)
        assert updated is True

        runs = get_loop_runs("loop-1", db_path=db_path)
        assert runs[0].status == LoopStatus.IDLE
        assert runs[0].ended_at is not None


def test_db_update_loop_run_step():
    """Update loop run's current step."""
    with tempfile.TemporaryDirectory() as tmpdir:
        db_path = Path(tmpdir) / "test.db"

        loop = Loop(
            id="loop-1",
            type=LoopType.LOOP,
            goal="test",
            repo=Path("/tmp/repo"),
            personal_main="test-main",
        )
        save_loop(loop, db_path)

        run = LoopRun(
            id="run-1",
            loop_id="loop-1",
            iteration=1,
            status=LoopStatus.RUNNING,
            started_at=datetime.now(),
        )
        save_loop_run(run, db_path)

        updated = update_loop_run_step("run-1", "implement", db_path)
        assert updated is True

        runs = get_loop_runs("loop-1", db_path=db_path)
        assert runs[0].current_step == "implement"


def test_db_update_loop_run_pr():
    """Update loop run's PR URL."""
    with tempfile.TemporaryDirectory() as tmpdir:
        db_path = Path(tmpdir) / "test.db"

        loop = Loop(
            id="loop-1",
            type=LoopType.LOOP,
            goal="test",
            repo=Path("/tmp/repo"),
            personal_main="test-main",
        )
        save_loop(loop, db_path)

        run = LoopRun(
            id="run-1",
            loop_id="loop-1",
            iteration=1,
            status=LoopStatus.RUNNING,
            started_at=datetime.now(),
        )
        save_loop_run(run, db_path)

        updated = update_loop_run_pr("run-1", "https://github.com/user/repo/pull/42", db_path)
        assert updated is True

        runs = get_loop_runs("loop-1", db_path=db_path)
        assert runs[0].pr_url == "https://github.com/user/repo/pull/42"


def test_db_update_loop_pid():
    """Update loop's process ID."""
    with tempfile.TemporaryDirectory() as tmpdir:
        db_path = Path(tmpdir) / "test.db"

        loop = Loop(
            id="loop-1",
            type=LoopType.LOOP,
            goal="test",
            repo=Path("/tmp/repo"),
            personal_main="test-main",
        )
        save_loop(loop, db_path)

        # Set pid
        updated = update_loop_pid("loop-1", 12345, db_path)
        assert updated is True

        loaded = get_loop("loop-1", db_path)
        assert loaded.pid == 12345

        # Clear pid
        updated = update_loop_pid("loop-1", None, db_path)
        assert updated is True

        loaded = get_loop("loop-1", db_path)
        assert loaded.pid is None


def test_loop_model_with_pid():
    """Loop model stores pid correctly."""
    loop = Loop(
        id="loop-1",
        type=LoopType.LOOP,
        goal="test",
        repo=Path("/tmp/repo"),
        personal_main="test-main",
        pid=12345,
    )
    assert loop.pid == 12345


def test_db_save_loop_with_pid():
    """Save and load loop with pid."""
    with tempfile.TemporaryDirectory() as tmpdir:
        db_path = Path(tmpdir) / "test.db"

        loop = Loop(
            id="loop-1",
            type=LoopType.LOOP,
            goal="test",
            repo=Path("/tmp/repo"),
            personal_main="test-main",
            pid=54321,
        )
        save_loop(loop, db_path)

        loaded = get_loop("loop-1", db_path)
        assert loaded.pid == 54321


# StartResult tests


def test_start_result_truthy_when_ok():
    """StartResult is truthy when ok=True."""
    from loopflow.lfd.loops import StartResult

    result = StartResult(True)
    assert result.ok is True
    assert result  # truthy
    assert result.reason is None
    assert result.outstanding is None


def test_start_result_falsy_when_not_ok():
    """StartResult is falsy when ok=False."""
    from loopflow.lfd.loops import StartResult

    result = StartResult(False, "already_running")
    assert result.ok is False
    assert not result  # falsy
    assert result.reason == "already_running"


def test_start_result_with_outstanding():
    """StartResult includes outstanding count for waiting state."""
    from loopflow.lfd.loops import StartResult

    result = StartResult(False, "waiting", outstanding=5)
    assert not result
    assert result.reason == "waiting"
    assert result.outstanding == 5
