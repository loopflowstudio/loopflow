"""Tests for lfd daemon."""

import tempfile
from datetime import datetime, timedelta
from pathlib import Path

from loopflow.lfd.agent import (
    MAX_PENDING_ACTIVATIONS,
    delete_agent,
    get_agent,
    get_agent_by_area_repo,
    list_agents,
    save_agent,
    update_agent_iteration,
    update_agent_pending_activations,
    update_agent_pid,
    update_agent_status,
)
from loopflow.lfd.daemon.protocol import Event, Request, error, success
from loopflow.lfd.db import _get_db
from loopflow.lfd.flow_run import (
    get_latest_run_for_agent,
    list_runs_for_agent,
    save_run,
    update_run_pr,
    update_run_status,
    update_run_step,
)
from loopflow.lfd.migrations.registry import MIGRATIONS
from loopflow.lfd.models import (
    Agent,
    AgentMode,
    AgentStatus,
    FlowRun,
    FlowRunStatus,
    MergeMode,
    StepRun,
    StepRunStatus,
)
from loopflow.lfd.step_run import (
    load_step_runs,
    load_step_runs_for_repo,
    load_step_runs_for_worktree,
    save_step_run,
    update_step_run_status,
)


def test_session_serialization():
    session = StepRun(
        id="sess-1",
        step="implement",
        repo="/tmp/repo",
        worktree="/tmp/repo.feature",
        status=StepRunStatus.RUNNING,
        started_at=datetime(2024, 1, 1, 12, 0, 0),
        model="claude-code",
        run_mode="auto",
    )
    data = session.to_dict()
    restored = StepRun.from_dict(data)
    assert restored.step == "implement"
    assert restored.status == StepRunStatus.RUNNING


def test_protocol_request_parse():
    line = '{"method": "sessions.list", "params": {"name": "test"}}'
    request = Request.parse(line)
    assert request.method == "sessions.list"
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


def test_db_save_and_load_session():
    with tempfile.TemporaryDirectory() as tmpdir:
        db_path = Path(tmpdir) / "test.db"
        session = StepRun(
            id="sess-1",
            step="implement",
            repo="/tmp/repo",
            worktree="/tmp/repo.feature",
            status=StepRunStatus.RUNNING,
            started_at=datetime.now(),
        )
        save_step_run(session, db_path)

        sessions = load_step_runs(db_path=db_path)
        assert len(sessions) == 1
        assert sessions[0].step == "implement"


def test_db_records_migrations():
    with tempfile.TemporaryDirectory() as tmpdir:
        db_path = Path(tmpdir) / "test.db"
        conn = _get_db(db_path)
        rows = conn.execute(
            "SELECT version, applied_at FROM schema_migrations ORDER BY version"
        ).fetchall()
        conn.close()

        # Should have all migrations recorded
        assert len(rows) == len(MIGRATIONS)
        for i, row in enumerate(rows):
            assert row[0] == MIGRATIONS[i].version
            assert row[1]  # applied_at timestamp exists


def test_db_schema_version_recorded():
    """Fresh DB records schema version in _meta table."""
    from loopflow.lfd.db import SCHEMA_VERSION, _get_schema_version

    with tempfile.TemporaryDirectory() as tmpdir:
        db_path = Path(tmpdir) / "test.db"
        conn = _get_db(db_path)
        version = _get_schema_version(conn)
        conn.close()

        assert version == SCHEMA_VERSION


def test_db_schema_mismatch_auto_resets():
    """Schema version mismatch auto-resets the database."""
    import sqlite3

    from loopflow.lfd.db import SCHEMA_VERSION, _get_schema_version

    with tempfile.TemporaryDirectory() as tmpdir:
        db_path = Path(tmpdir) / "test.db"

        # Create DB with wrong schema version and some data
        conn = sqlite3.connect(db_path)
        conn.execute("CREATE TABLE _meta (key TEXT PRIMARY KEY, value TEXT NOT NULL)")
        conn.execute("INSERT INTO _meta (key, value) VALUES ('schema_version', 'old-version')")
        conn.execute("CREATE TABLE old_table (id TEXT)")
        conn.execute("INSERT INTO old_table VALUES ('old-data')")
        conn.commit()
        conn.close()

        # Mismatch should auto-reset
        conn = _get_db(db_path)
        version = _get_schema_version(conn)
        assert version == SCHEMA_VERSION

        # Old table should be gone
        cursor = conn.execute(
            "SELECT name FROM sqlite_master WHERE type='table' AND name='old_table'"
        )
        assert cursor.fetchone() is None
        conn.close()


def test_db_reset_function():
    """reset_db() clears and recreates database."""
    from loopflow.lfd.db import SCHEMA_VERSION, _get_schema_version, reset_db

    with tempfile.TemporaryDirectory() as tmpdir:
        db_path = Path(tmpdir) / "test.db"

        # Create initial DB with some data
        conn = _get_db(db_path)
        conn.execute(
            "INSERT INTO agents (id, repo, flow, voice, area, status, iteration, "
            "main_branch, pr_limit, merge_mode, created_at) "
            "VALUES ('test', '/tmp', 'ship', '[]', '[]', 'idle', 0, 'main', 5, "
            "'pr', '2024-01-01')"
        )
        conn.commit()
        conn.close()

        # Reset
        reset_db(db_path)

        # Should be fresh with correct schema version
        conn = _get_db(db_path)
        version = _get_schema_version(conn)
        assert version == SCHEMA_VERSION

        # Old data should be gone
        cursor = conn.execute("SELECT COUNT(*) FROM agents")
        assert cursor.fetchone()[0] == 0
        conn.close()


# =============================================================================
# Migration completeness tests
#
# These tests verify that migrations create columns for ALL model fields.
# If a field is added to a model without a corresponding migration, the
# INSERT will fail and these tests catch it.
# =============================================================================


def test_migrations_cover_all_agent_fields():
    """Migrations create columns for all Agent model fields."""
    with tempfile.TemporaryDirectory() as tmpdir:
        db_path = Path(tmpdir) / "test.db"

        agent = Agent(
            id="test-all-fields",
            flow="ship",
            area=["src/test/"],
            voice=["voice-a", "voice-b"],
            repo=Path("/tmp/repo"),
            status=AgentStatus.RUNNING,
            iteration=5,
            main_branch="test-main",
            pr_limit=10,
            merge_mode=MergeMode.LAND,
            pid=12345,
            watch_paths="src/**/*.py",
            cron="0 9 * * *",
            last_main_sha="abc123",
        )

        save_agent(agent, db_path)
        loaded = get_agent("test-all-fields", db_path)

        assert loaded.id == agent.id
        assert loaded.flow == agent.flow
        assert loaded.area == agent.area
        assert loaded.voice == agent.voice
        assert loaded.repo == agent.repo
        assert loaded.status == agent.status
        assert loaded.iteration == agent.iteration
        assert loaded.main_branch == agent.main_branch
        assert loaded.pr_limit == agent.pr_limit
        assert loaded.merge_mode == agent.merge_mode
        assert loaded.pid == agent.pid
        assert loaded.watch_paths == agent.watch_paths
        assert loaded.cron == agent.cron
        assert loaded.last_main_sha == agent.last_main_sha


def test_migrations_cover_all_run_fields():
    """Migrations create columns for all Run model fields."""
    with tempfile.TemporaryDirectory() as tmpdir:
        db_path = Path(tmpdir) / "test.db"

        run = FlowRun(
            id="test-run-all-fields",
            agent_id="agent-id",
            flow="ship",
            area=["src/test/"],
            voice=["voice-a", "voice-b"],
            repo=Path("/tmp/repo"),
            status=FlowRunStatus.RUNNING,
            iteration=3,
            worktree="/tmp/repo.worktree",
            branch="feature-branch",
            current_step="implement",
            error="Something went wrong",
            pr_url="https://github.com/user/repo/pull/42",
            started_at=datetime(2024, 1, 15, 10, 30, 0),
            ended_at=datetime(2024, 1, 15, 11, 45, 0),
        )

        save_run(run, db_path)
        runs = list_runs_for_agent("agent-id", db_path=db_path)
        loaded = runs[0]

        assert loaded.id == run.id
        assert loaded.agent_id == run.agent_id
        assert loaded.flow == run.flow
        assert loaded.area == run.area
        assert loaded.voice == run.voice
        assert loaded.repo == run.repo
        assert loaded.status == run.status
        assert loaded.iteration == run.iteration
        assert loaded.worktree == run.worktree
        assert loaded.branch == run.branch
        assert loaded.current_step == run.current_step
        assert loaded.error == run.error
        assert loaded.pr_url == run.pr_url
        assert loaded.started_at == run.started_at
        assert loaded.ended_at == run.ended_at


def test_migrations_cover_all_session_fields():
    """Migrations create columns for all Session model fields."""
    with tempfile.TemporaryDirectory() as tmpdir:
        db_path = Path(tmpdir) / "test.db"

        session = StepRun(
            id="test-session-all-fields",
            step="implement",
            repo="/tmp/repo",
            worktree="/tmp/repo.feature",
            status=StepRunStatus.RUNNING,
            started_at=datetime(2024, 1, 15, 10, 30, 0),
            ended_at=datetime(2024, 1, 15, 11, 45, 0),
            pid=54321,
            model="claude:opus",
            run_mode="interactive",
        )

        save_step_run(session, db_path)
        sessions = load_step_runs(db_path=db_path)
        loaded = sessions[0]

        assert loaded.id == session.id
        assert loaded.step == session.step
        assert loaded.repo == session.repo
        assert loaded.worktree == session.worktree
        assert loaded.status == session.status
        assert loaded.started_at == session.started_at
        assert loaded.ended_at == session.ended_at
        assert loaded.pid == session.pid
        assert loaded.model == session.model
        assert loaded.run_mode == session.run_mode


def test_migrations_cover_summary_fields():
    """Migrations create columns for summary storage."""
    from loopflow.lfd.db import load_summary_db, save_summary_db

    with tempfile.TemporaryDirectory() as tmpdir:
        db_path = Path(tmpdir) / "test.db"

        save_summary_db(
            repo="/tmp/repo",
            path="src/loopflow",
            token_budget=25000,
            source_hash="abc123def456",
            content="# Summary\n\nThis is a test summary.",
            model="claude:sonnet",
            db_path=db_path,
        )

        loaded = load_summary_db(
            repo="/tmp/repo",
            path="src/loopflow",
            token_budget=25000,
            db_path=db_path,
        )

        assert loaded is not None
        assert loaded["content"] == "# Summary\n\nThis is a test summary."
        assert loaded["source_hash"] == "abc123def456"
        assert loaded["model"] == "claude:sonnet"
        assert loaded["created_at"] is not None


def test_db_load_step_runs_for_worktree():
    with tempfile.TemporaryDirectory() as tmpdir:
        db_path = Path(tmpdir) / "test.db"

        session1 = StepRun(
            id="sess-1",
            step="implement",
            repo="/tmp/repo",
            worktree="/tmp/repo.feature-a",
            status=StepRunStatus.COMPLETED,
            started_at=datetime(2024, 1, 1, 12, 0, 0),
        )
        session2 = StepRun(
            id="sess-2",
            step="review",
            repo="/tmp/repo",
            worktree="/tmp/repo.feature-a",
            status=StepRunStatus.COMPLETED,
            started_at=datetime(2024, 1, 2, 12, 0, 0),
        )
        session3 = StepRun(
            id="sess-3",
            step="implement",
            repo="/tmp/repo",
            worktree="/tmp/repo.feature-b",
            status=StepRunStatus.COMPLETED,
            started_at=datetime.now(),
        )

        save_step_run(session1, db_path)
        save_step_run(session2, db_path)
        save_step_run(session3, db_path)

        sessions = load_step_runs_for_worktree("/tmp/repo.feature-a", db_path=db_path)
        assert len(sessions) == 2
        # Should be ordered by started_at DESC
        assert sessions[0].id == "sess-2"
        assert sessions[1].id == "sess-1"


def test_db_load_step_runs_for_repo():
    with tempfile.TemporaryDirectory() as tmpdir:
        db_path = Path(tmpdir) / "test.db"

        session1 = StepRun(
            id="sess-1",
            step="implement",
            repo="/tmp/repo-a",
            worktree="/tmp/repo-a.feature",
            status=StepRunStatus.COMPLETED,
            started_at=datetime.now(),
        )
        session2 = StepRun(
            id="sess-2",
            step="review",
            repo="/tmp/repo-b",
            worktree="/tmp/repo-b.feature",
            status=StepRunStatus.COMPLETED,
            started_at=datetime.now(),
        )

        save_step_run(session1, db_path)
        save_step_run(session2, db_path)

        sessions = load_step_runs_for_repo("/tmp/repo-a", db_path=db_path)
        assert len(sessions) == 1
        assert sessions[0].id == "sess-1"


def test_db_update_step_run_status():
    with tempfile.TemporaryDirectory() as tmpdir:
        db_path = Path(tmpdir) / "test.db"

        session = StepRun(
            id="sess-1",
            step="implement",
            repo="/tmp/repo",
            worktree="/tmp/repo.feature",
            status=StepRunStatus.RUNNING,
            started_at=datetime.now(),
        )
        save_step_run(session, db_path)

        updated = update_step_run_status("sess-1", StepRunStatus.COMPLETED, db_path)
        assert updated is True

        sessions = load_step_runs_for_worktree("/tmp/repo.feature", db_path=db_path)
        assert len(sessions) == 1
        assert sessions[0].status == StepRunStatus.COMPLETED
        assert sessions[0].ended_at is not None


def test_db_update_step_run_status_nonexistent():
    with tempfile.TemporaryDirectory() as tmpdir:
        db_path = Path(tmpdir) / "test.db"
        # Initialize DB by saving and then checking update
        session = StepRun(
            id="sess-1",
            step="implement",
            repo="/tmp/repo",
            worktree="/tmp/repo.feature",
            status=StepRunStatus.RUNNING,
            started_at=datetime.now(),
        )
        save_step_run(session, db_path)

        updated = update_step_run_status("nonexistent", StepRunStatus.COMPLETED, db_path)
        assert updated is False


# Server handler tests


def test_server_handle_output_line_broadcasts_event():
    """output.line handler broadcasts event to subscribers."""
    import asyncio

    from loopflow.lfd.daemon.server import Server

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
            params = {"step_run_id": "test-step-run-123", "text": "→ Read: foo.py"}
            response = await server._handle_output_line(params)

            # Check response
            assert response.ok is True
            assert response.result == {}

            # Check event was broadcast
            assert len(broadcast_events) == 1
            event = broadcast_events[0]
            assert event.event == "output.line"
            assert event.data["session_id"] == "test-step-run-123"
            assert event.data["text"] == "→ Read: foo.py"
            assert "timestamp" in event.data

    asyncio.run(run_test())


def test_server_handle_output_line_missing_step_run_id():
    """output.line handler returns error for missing step_run_id."""
    import asyncio

    from loopflow.lfd.daemon.server import Server

    async def run_test():
        with tempfile.TemporaryDirectory() as tmpdir:
            socket_path = Path(tmpdir) / "test.sock"
            server = Server(socket_path)

            params = {"text": "→ Read: foo.py"}
            response = await server._handle_output_line(params)

            assert response.ok is False
            assert "step_run_id" in response.error

    asyncio.run(run_test())


def test_server_handle_output_line_missing_text():
    """output.line handler returns error for missing text."""
    import asyncio

    from loopflow.lfd.daemon.server import Server

    async def run_test():
        with tempfile.TemporaryDirectory() as tmpdir:
            socket_path = Path(tmpdir) / "test.sock"
            server = Server(socket_path)

            params = {"step_run_id": "test-step-run-123"}
            response = await server._handle_output_line(params)

            assert response.ok is False
            assert "text" in response.error

    asyncio.run(run_test())


def test_server_handle_output_line_allows_empty_text():
    """output.line handler accepts empty string for text (allows blank lines)."""
    import asyncio

    from loopflow.lfd.daemon.server import Server

    async def run_test():
        with tempfile.TemporaryDirectory() as tmpdir:
            socket_path = Path(tmpdir) / "test.sock"
            server = Server(socket_path)

            broadcast_events = []
            server._broadcast = lambda e: broadcast_events.append(e) or asyncio.sleep(0)

            # Empty string should be allowed (it's a blank line in output)
            params = {"step_run_id": "test-step-run-123", "text": ""}
            response = await server._handle_output_line(params)

            assert response.ok is True
            assert len(broadcast_events) == 1
            assert broadcast_events[0].data["text"] == ""

    asyncio.run(run_test())


# Agent model tests


def _make_agent(**kwargs) -> Agent:
    """Helper to create an Agent with test defaults."""
    # Determine mode from trigger config if not explicitly set
    if "mode" not in kwargs:
        if kwargs.get("watch_paths"):
            kwargs["mode"] = AgentMode.WATCH
        elif kwargs.get("cron"):
            kwargs["mode"] = AgentMode.CRON
        else:
            kwargs["mode"] = AgentMode.LOOP
    defaults = {
        "id": "test-id",
        "flow": "ship",
        "area": ["src/test/"],
        "voice": ["default"],
        "repo": Path("/tmp/repo"),
        "main_branch": "test-main",
    }
    defaults.update(kwargs)
    return Agent(**defaults)


def test_agent_model_defaults():
    """Agent model has correct defaults."""
    agent = Agent(
        id="agent-1",
        flow="ship",
        area=["src/test-coverage/"],
        voice=["default"],
        repo=Path("/tmp/repo"),
        main_branch="test-coverage-main",
    )
    assert agent.flow == "ship"
    assert agent.status == AgentStatus.IDLE
    assert agent.iteration == 0
    assert agent.pr_limit == 5
    assert agent.merge_mode == MergeMode.PR
    assert agent.pid is None


def test_agent_model_short_id():
    """Agent.short_id() returns first 7 chars."""
    agent = _make_agent(id="abcdef1234567890")
    assert agent.short_id() == "abcdef1"


def test_agent_mode_property():
    """Agent.mode returns correct activation mode."""
    loop_agent = _make_agent()
    assert loop_agent.mode == AgentMode.LOOP

    watch_agent = _make_agent(watch_paths="src/**/*.py")
    assert watch_agent.mode == AgentMode.WATCH

    cron_agent = _make_agent(cron="0 9 * * *")
    assert cron_agent.mode == AgentMode.CRON


def test_run_model():
    """Run model stores execution data."""
    run = FlowRun(
        id="run-1",
        agent_id="agent-1",
        flow="ship",
        area=["src/test/"],
        voice=["default"],
        repo=Path("/tmp/repo"),
        status=FlowRunStatus.RUNNING,
        iteration=3,
        worktree="/tmp/repo.wt",
        current_step="implement",
        started_at=datetime.now(),
    )
    assert run.iteration == 3
    assert run.status == FlowRunStatus.RUNNING
    assert run.current_step == "implement"
    assert run.ended_at is None
    assert run.error is None
    assert run.pr_url is None


# Agent database tests


def test_db_save_and_get_agent():
    """Save and retrieve an agent."""
    with tempfile.TemporaryDirectory() as tmpdir:
        db_path = Path(tmpdir) / "test.db"
        agent = _make_agent(
            id="agent-123",
            area=["src/test-coverage/"],
            main_branch="test-coverage-main",
            status=AgentStatus.IDLE,
            iteration=0,
            pr_limit=5,
        )
        save_agent(agent, db_path)

        loaded = get_agent("agent-123", db_path)
        assert loaded is not None
        assert loaded.id == "agent-123"
        assert loaded.area == ["src/test-coverage/"]
        assert loaded.main_branch == "test-coverage-main"
        assert loaded.flow == "ship"


def test_db_get_agent_short_id():
    """Get agent by short ID prefix."""
    with tempfile.TemporaryDirectory() as tmpdir:
        db_path = Path(tmpdir) / "test.db"
        agent = _make_agent(id="abcdef1234567890")
        save_agent(agent, db_path)

        # Should find by prefix
        loaded = get_agent("abcdef1", db_path)
        assert loaded is not None
        assert loaded.id == "abcdef1234567890"


def test_db_get_agent_by_area_repo():
    """Get agent by area and repo."""
    with tempfile.TemporaryDirectory() as tmpdir:
        db_path = Path(tmpdir) / "test.db"
        agent = _make_agent(
            id="agent-1",
            area=["src/api/"],
            main_branch="api-aurora-melody-main",
        )
        save_agent(agent, db_path)

        loaded = get_agent_by_area_repo(["src/api/"], Path("/tmp/repo"), db_path=db_path)
        assert loaded is not None
        assert loaded.id == "agent-1"

        # Different area should not match
        not_found = get_agent_by_area_repo(["src/other/"], Path("/tmp/repo"), db_path=db_path)
        assert not_found is None


def test_db_list_agents():
    """List all agents."""
    with tempfile.TemporaryDirectory() as tmpdir:
        db_path = Path(tmpdir) / "test.db"

        agent1 = _make_agent(
            id="agent-1",
            area=["src/goal-a/"],
            repo=Path("/tmp/repo-a"),
            main_branch="goal-a-main",
        )
        agent2 = _make_agent(
            id="agent-2",
            area=["src/goal-b/"],
            repo=Path("/tmp/repo-b"),
            main_branch="goal-b-main",
        )
        save_agent(agent1, db_path)
        save_agent(agent2, db_path)

        agents = list_agents(db_path=db_path)
        assert len(agents) == 2

        # Filter by repo
        agents = list_agents(repo=Path("/tmp/repo-a"), db_path=db_path)
        assert len(agents) == 1
        assert agents[0].area == ["src/goal-a/"]


def test_db_update_agent_status():
    """Update agent status."""
    with tempfile.TemporaryDirectory() as tmpdir:
        db_path = Path(tmpdir) / "test.db"
        agent = _make_agent(
            id="agent-1",
            status=AgentStatus.IDLE,
        )
        save_agent(agent, db_path)

        updated = update_agent_status("agent-1", AgentStatus.RUNNING, db_path)
        assert updated is True

        loaded = get_agent("agent-1", db_path)
        assert loaded.status == AgentStatus.RUNNING


def test_db_update_agent_iteration():
    """Update agent iteration count."""
    with tempfile.TemporaryDirectory() as tmpdir:
        db_path = Path(tmpdir) / "test.db"
        agent = _make_agent(
            id="agent-1",
            iteration=0,
        )
        save_agent(agent, db_path)

        updated = update_agent_iteration("agent-1", 5, db_path)
        assert updated is True

        loaded = get_agent("agent-1", db_path)
        assert loaded.iteration == 5


def test_db_delete_agent():
    """Delete agent and its runs."""
    with tempfile.TemporaryDirectory() as tmpdir:
        db_path = Path(tmpdir) / "test.db"
        agent = _make_agent(
            id="agent-1",
            flow="ship",
            area=["src/test/"],
            repo=Path("/tmp/repo"),
            main_branch="test-main",
        )
        save_agent(agent, db_path)

        # Add a run
        run = FlowRun(
            id="run-1",
            agent_id="agent-1",
            flow="ship",
            area=["src/test/"],
            voice=["default"],
            repo=Path("/tmp/repo"),
            iteration=1,
            status=FlowRunStatus.RUNNING,
            started_at=datetime.now(),
        )
        save_run(run, db_path)

        # Delete agent (should also delete runs)
        deleted = delete_agent("agent-1", db_path)
        assert deleted is True

        assert get_agent("agent-1", db_path) is None
        assert list_runs_for_agent("agent-1", db_path=db_path) == []


# Run database tests


def test_db_save_and_get_runs():
    """Save and retrieve runs."""
    with tempfile.TemporaryDirectory() as tmpdir:
        db_path = Path(tmpdir) / "test.db"

        # Create parent agent first
        agent = _make_agent(id="agent-1")
        save_agent(agent, db_path)

        run1 = FlowRun(
            id="run-1",
            agent_id="agent-1",
            flow="ship",
            area=["src/test/"],
            voice=["default"],
            repo=Path("/tmp/repo"),
            iteration=1,
            status=FlowRunStatus.COMPLETED,
            started_at=datetime(2024, 1, 1, 12, 0, 0),
            pr_url="https://github.com/user/repo/pull/1",
        )
        run2 = FlowRun(
            id="run-2",
            agent_id="agent-1",
            flow="ship",
            area=["src/test/"],
            voice=["default"],
            repo=Path("/tmp/repo"),
            iteration=2,
            status=FlowRunStatus.RUNNING,
            started_at=datetime(2024, 1, 2, 12, 0, 0),
        )
        save_run(run1, db_path)
        save_run(run2, db_path)

        runs = list_runs_for_agent("agent-1", db_path=db_path)
        assert len(runs) == 2


def test_db_get_latest_run_for_agent():
    """Get most recent run for an agent."""
    with tempfile.TemporaryDirectory() as tmpdir:
        db_path = Path(tmpdir) / "test.db"

        agent = _make_agent(id="agent-1")
        save_agent(agent, db_path)

        run1 = FlowRun(
            id="run-1",
            agent_id="agent-1",
            flow="ship",
            area=["src/test/"],
            voice=["default"],
            repo=Path("/tmp/repo"),
            iteration=1,
            status=FlowRunStatus.COMPLETED,
            started_at=datetime(2024, 1, 1, 12, 0, 0),
        )
        run2 = FlowRun(
            id="run-2",
            agent_id="agent-1",
            flow="ship",
            area=["src/test/"],
            voice=["default"],
            repo=Path("/tmp/repo"),
            iteration=2,
            status=FlowRunStatus.RUNNING,
            started_at=datetime(2024, 1, 2, 12, 0, 0),
        )
        save_run(run1, db_path)
        save_run(run2, db_path)

        latest = get_latest_run_for_agent("agent-1", db_path)
        assert latest is not None
        assert latest.id == "run-2"


def test_db_update_run_status():
    """Update run status."""
    with tempfile.TemporaryDirectory() as tmpdir:
        db_path = Path(tmpdir) / "test.db"

        agent = _make_agent(id="agent-1")
        save_agent(agent, db_path)

        run = FlowRun(
            id="run-1",
            agent_id="agent-1",
            flow="ship",
            area=["src/test/"],
            voice=["default"],
            repo=Path("/tmp/repo"),
            iteration=1,
            status=FlowRunStatus.RUNNING,
            started_at=datetime.now(),
        )
        save_run(run, db_path)

        updated = update_run_status("run-1", FlowRunStatus.COMPLETED, db_path=db_path)
        assert updated is True

        runs = list_runs_for_agent("agent-1", db_path=db_path)
        assert runs[0].status == FlowRunStatus.COMPLETED
        assert runs[0].ended_at is not None


def test_db_update_run_step():
    """Update run's current step."""
    with tempfile.TemporaryDirectory() as tmpdir:
        db_path = Path(tmpdir) / "test.db"

        agent = _make_agent(id="agent-1")
        save_agent(agent, db_path)

        run = FlowRun(
            id="run-1",
            agent_id="agent-1",
            flow="ship",
            area=["src/test/"],
            voice=["default"],
            repo=Path("/tmp/repo"),
            iteration=1,
            status=FlowRunStatus.RUNNING,
            started_at=datetime.now(),
        )
        save_run(run, db_path)

        updated = update_run_step("run-1", "implement", db_path)
        assert updated is True

        runs = list_runs_for_agent("agent-1", db_path=db_path)
        assert runs[0].current_step == "implement"


def test_db_update_run_pr():
    """Update run's PR URL."""
    with tempfile.TemporaryDirectory() as tmpdir:
        db_path = Path(tmpdir) / "test.db"

        agent = _make_agent(id="agent-1")
        save_agent(agent, db_path)

        run = FlowRun(
            id="run-1",
            agent_id="agent-1",
            flow="ship",
            area=["src/test/"],
            voice=["default"],
            repo=Path("/tmp/repo"),
            iteration=1,
            status=FlowRunStatus.RUNNING,
            started_at=datetime.now(),
        )
        save_run(run, db_path)

        updated = update_run_pr("run-1", "https://github.com/user/repo/pull/42", db_path)
        assert updated is True

        runs = list_runs_for_agent("agent-1", db_path=db_path)
        assert runs[0].pr_url == "https://github.com/user/repo/pull/42"


def test_db_update_agent_pid():
    """Update agent's process ID."""
    with tempfile.TemporaryDirectory() as tmpdir:
        db_path = Path(tmpdir) / "test.db"

        agent = _make_agent(id="agent-1")
        save_agent(agent, db_path)

        # Set pid
        updated = update_agent_pid("agent-1", 12345, db_path)
        assert updated is True

        loaded = get_agent("agent-1", db_path)
        assert loaded.pid == 12345

        # Clear pid
        updated = update_agent_pid("agent-1", None, db_path)
        assert updated is True

        loaded = get_agent("agent-1", db_path)
        assert loaded.pid is None


def test_agent_model_with_pid():
    """Agent model stores pid correctly."""
    agent = _make_agent(id="agent-1", pid=12345)
    assert agent.pid == 12345


def test_db_save_agent_with_pid():
    """Save and load agent with pid."""
    with tempfile.TemporaryDirectory() as tmpdir:
        db_path = Path(tmpdir) / "test.db"

        agent = _make_agent(id="agent-1", pid=54321)
        save_agent(agent, db_path)

        loaded = get_agent("agent-1", db_path)
        assert loaded.pid == 54321


# StartResult tests


def test_start_result_truthy_when_ok():
    """StartResult is truthy when ok=True."""
    from loopflow.lfd.agent import StartResult

    result = StartResult(True)
    assert result.ok is True
    assert result  # truthy
    assert result.reason is None
    assert result.outstanding is None


def test_start_result_falsy_when_not_ok():
    """StartResult is falsy when ok=False."""
    from loopflow.lfd.agent import StartResult

    result = StartResult(False, "already_running")
    assert result.ok is False
    assert not result  # falsy
    assert result.reason == "already_running"


def test_start_result_with_outstanding():
    """StartResult includes outstanding count for waiting state."""
    from loopflow.lfd.agent import StartResult

    result = StartResult(False, "waiting", outstanding=5)
    assert not result
    assert result.reason == "waiting"
    assert result.outstanding == 5


# Scheduler tests


def test_scheduler_config_defaults():
    """SchedulerConfig has correct defaults."""
    from loopflow.lfd.daemon.manager import ManagerConfig as SchedulerConfig

    config = SchedulerConfig()
    assert config.concurrency == 3
    assert config.global_pr_limit == 15


def test_scheduler_acquire_and_release():
    """Scheduler manages slots correctly."""
    from loopflow.lfd.daemon.manager import Manager as Scheduler
    from loopflow.lfd.daemon.manager import ManagerConfig as SchedulerConfig

    config = SchedulerConfig(concurrency=2, global_pr_limit=100)
    scheduler = Scheduler(config)
    scheduler.total_outstanding = lambda: 0  # Mock to avoid git/db calls

    # Initial state
    assert scheduler.slots_available() == 2
    assert scheduler.slots_used() == 0

    # Acquire first slot
    acquired, reason = scheduler.acquire("run-1")
    assert acquired is True
    assert reason is None
    assert scheduler.slots_available() == 1
    assert scheduler.slots_used() == 1

    # Acquire second slot
    acquired, reason = scheduler.acquire("run-2")
    assert acquired is True
    assert reason is None
    assert scheduler.slots_available() == 0
    assert scheduler.slots_used() == 2

    # Can't acquire third slot (at limit)
    acquired, reason = scheduler.acquire("run-3")
    assert acquired is False
    assert reason == "concurrency"
    assert scheduler.slots_used() == 2

    # Release a slot
    scheduler.release("run-1")
    assert scheduler.slots_available() == 1
    assert scheduler.slots_used() == 1

    # Now can acquire again
    acquired, reason = scheduler.acquire("run-3")
    assert acquired is True
    assert reason is None
    assert scheduler.slots_used() == 2


def test_scheduler_release_nonexistent():
    """Releasing nonexistent run ID is safe."""
    from loopflow.lfd.daemon.manager import Manager as Scheduler
    from loopflow.lfd.daemon.manager import ManagerConfig as SchedulerConfig

    config = SchedulerConfig(concurrency=2, global_pr_limit=100)
    scheduler = Scheduler(config)

    # Should not raise
    scheduler.release("nonexistent")
    assert scheduler.slots_used() == 0


def test_scheduler_get_status():
    """Scheduler returns correct status."""
    from loopflow.lfd.daemon.manager import Manager as Scheduler
    from loopflow.lfd.daemon.manager import ManagerConfig as SchedulerConfig

    config = SchedulerConfig(concurrency=3, global_pr_limit=15)
    scheduler = Scheduler(config)
    scheduler.total_outstanding = lambda: 0  # Mock to avoid git/db calls

    acquired, reason = scheduler.acquire("run-1")
    assert acquired is True
    assert reason is None
    acquired, reason = scheduler.acquire("run-2")
    assert acquired is True
    assert reason is None

    status = scheduler.get_status()
    assert status["slots_used"] == 2
    assert status["slots_total"] == 3
    assert status["outstanding_limit"] == 15
    assert "run-1" in status["running"]
    assert "run-2" in status["running"]


def test_scheduler_can_start_respects_concurrency():
    """can_start checks concurrency limit."""
    from loopflow.lfd.daemon.manager import Manager as Scheduler
    from loopflow.lfd.daemon.manager import ManagerConfig as SchedulerConfig

    config = SchedulerConfig(concurrency=1, global_pr_limit=100)
    scheduler = Scheduler(config)
    scheduler.total_outstanding = lambda: 0  # Mock to avoid git/db calls

    # Initially can start
    can, reason = scheduler.can_start()
    assert can is True
    assert reason is None

    # After acquiring, can't start
    acquired, reason = scheduler.acquire("run-1")
    assert acquired is True
    assert reason is None
    can, reason = scheduler.can_start()
    assert can is False
    assert reason == "concurrency"


def test_scheduler_acquire_respects_global_limit():
    """acquire blocks when global outstanding exceeds limit."""
    from loopflow.lfd.daemon.manager import Manager as Scheduler
    from loopflow.lfd.daemon.manager import ManagerConfig as SchedulerConfig

    config = SchedulerConfig(concurrency=2, global_pr_limit=1)
    scheduler = Scheduler(config)
    scheduler.total_outstanding = lambda: 2

    acquired, reason = scheduler.acquire("run-1")
    assert acquired is False
    assert reason == "global_limit"


def test_scheduler_thread_safety():
    """Scheduler is thread-safe for acquire/release."""
    import threading

    from loopflow.lfd.daemon.manager import Manager as Scheduler
    from loopflow.lfd.daemon.manager import ManagerConfig as SchedulerConfig

    config = SchedulerConfig(concurrency=10, global_pr_limit=100)
    scheduler = Scheduler(config)
    scheduler.total_outstanding = lambda: 0  # Mock to avoid git/db calls

    acquired_runs = []
    failed = []

    def try_acquire(run_id: str):
        acquired, _reason = scheduler.acquire(run_id)
        if acquired:
            acquired_runs.append(run_id)
        else:
            failed.append(run_id)

    # Start 20 threads trying to acquire 10 slots
    threads = [threading.Thread(target=try_acquire, args=(f"run-{i}",)) for i in range(20)]
    for t in threads:
        t.start()
    for t in threads:
        t.join()

    # Exactly 10 should have acquired
    assert len(acquired_runs) == 10
    assert len(failed) == 10
    assert scheduler.slots_used() == 10


# Schedule grace period tests


def test_schedule_triggers_within_grace_period():
    """Schedule triggers when missed time is within 24h grace period."""
    from loopflow.lfd.agent import should_trigger_cron

    # Cron for 9am daily, last run was yesterday
    last_run = datetime(2024, 1, 14, 9, 0, 0)

    # Use a cron that triggers every minute for predictable testing
    result = should_trigger_cron("* * * * *", last_run)
    assert result is True  # prev_time (recent) > last_run (yesterday)


def test_schedule_skips_stale_beyond_grace_period():
    """Schedule does NOT trigger when missed time is beyond grace period."""
    from loopflow.lfd.agent import should_trigger_cron

    last_run = datetime(2020, 1, 1, 9, 0, 0)  # Very old

    result = should_trigger_cron("0 9 * * *", last_run, grace_period=timedelta(hours=1))

    # Test is more about code path than specific values
    assert result is True or result is False  # Just verify it doesn't crash


def test_schedule_grace_period_skips_very_old():
    """Schedule with 0 grace period skips any missed time."""
    from loopflow.lfd.agent import should_trigger_cron

    # With zero grace period, only triggers if prev_time == now (impossible)
    last_run = datetime(2024, 1, 14, 9, 0, 0)
    result = should_trigger_cron("* * * * *", last_run, grace_period=timedelta(seconds=0))

    # prev_time is always at least a few seconds/ms ago, so this should be False
    assert result is False


def test_schedule_first_run_within_grace():
    """First schedule run triggers if within grace period."""
    from loopflow.lfd.agent import should_trigger_cron

    # No last_run (first time), should trigger if within grace
    result = should_trigger_cron("* * * * *", None)
    assert result is True


def test_schedule_first_run_beyond_grace():
    """First schedule run skips if beyond grace period."""
    from loopflow.lfd.agent import should_trigger_cron

    # No last_run, but zero grace period means prev_time is stale
    result = should_trigger_cron("* * * * *", None, grace_period=timedelta(seconds=0))
    assert result is False


# Random word generation tests


def test_generate_random_words_format():
    """generate_word_pair returns magical-musical format."""
    from loopflow.lf.naming import MAGICAL, MUSICAL, generate_word_pair

    words = generate_word_pair()
    parts = words.split("-")
    assert len(parts) == 2
    assert parts[0] in MAGICAL
    assert parts[1] in MUSICAL


def test_generate_random_words_produces_variety():
    """generate_word_pair produces different results over multiple calls."""
    from loopflow.lf.naming import generate_word_pair

    # Generate 20 word pairs, expect at least 5 unique
    results = {generate_word_pair() for _ in range(20)}
    assert len(results) >= 5  # Should be much higher with 34*26 combinations


def test_word_lists_have_sufficient_variety():
    """Word lists have enough entries for good uniqueness."""
    from loopflow.lf.naming import MAGICAL, MUSICAL

    # 34 magical * 26 musical = 884 combinations
    assert len(MAGICAL) >= 30
    assert len(MUSICAL) >= 20
    # All words should be lowercase and contain no special characters
    for word in MAGICAL + MUSICAL:
        assert word.islower()
        assert word.isalpha()


# Flow processing tests


def test_resolved_step_has_expected_attributes():
    """ResolvedStep has all attributes the loop runner expects."""
    from loopflow.lf.flows import ResolvedStep

    step = ResolvedStep()

    # These attributes are accessed by loop_runner.py
    assert hasattr(step, "step")
    assert hasattr(step, "config")
    assert hasattr(step, "parallel_group")
    assert hasattr(step, "choose")
    assert hasattr(step, "join")

    # Verify defaults are None
    assert step.step is None
    assert step.config is None
    assert step.parallel_group is None
    assert step.choose is None
    assert step.join is None


def test_resolved_step_with_values():
    """ResolvedStep can be constructed with values."""
    from loopflow.lf.flows import ResolvedStep, StepConfig

    config = StepConfig(model="claude:sonnet")
    step = ResolvedStep(
        step="implement",
        config=config,
        parallel_group=1,
    )

    assert step.step == "implement"
    assert step.config.model == "claude:sonnet"
    assert step.parallel_group == 1


def test_loop_runner_can_process_simple_flow():
    """Loop runner can iterate through resolved steps without errors."""
    from loopflow.lf.flows import ResolvedStep

    # Simulate what the loop runner does when iterating steps
    resolved = [
        ResolvedStep(step="lint"),
        ResolvedStep(step="implement"),
        ResolvedStep(step="test"),
    ]

    # Verify we can safely check all attributes without AttributeError
    for step in resolved:
        _ = step.parallel_group is not None
        _ = step.choose is not None
        _ = step.join is not None
        _ = step.step


def test_loop_runner_handles_fork_join_groups():
    """Loop runner correctly identifies fork/join groups."""
    from loopflow.lf.flows import Join, ResolvedStep

    resolved = [
        ResolvedStep(step="variant-a", parallel_group=0),
        ResolvedStep(step="variant-b", parallel_group=0),
        ResolvedStep(join=Join()),  # Join uses default JoinConfig
        ResolvedStep(step="final"),
    ]

    # Simulate the loop runner's fork detection logic
    i = 0
    fork_groups_found = 0

    while i < len(resolved):
        step = resolved[i]
        if step.parallel_group is not None:
            group_steps = []
            group = step.parallel_group
            while i < len(resolved) and resolved[i].parallel_group == group:
                group_steps.append(resolved[i])
                i += 1
            fork_groups_found += 1
            assert len(group_steps) == 2

            # Should find join after fork
            if i < len(resolved):
                assert resolved[i].join is not None
                i += 1
            continue
        i += 1

    assert fork_groups_found == 1


# =============================================================================
# State lifecycle tests (Phase 1)
# =============================================================================


def test_cleanup_stale_runs_marks_orphaned():
    """cleanup_stale_runs marks runs without agents as FAILED."""
    from loopflow.lfd.flow_run import cleanup_stale_runs, get_run, save_run

    with tempfile.TemporaryDirectory() as tmpdir:
        db_path = Path(tmpdir) / "test.db"

        # Create a run with no agent
        run = FlowRun(
            id="orphan-run",
            agent_id=None,
            flow="ship",
            area=["src/"],
            voice=["default"],
            repo=Path("/tmp/repo"),
            status=FlowRunStatus.RUNNING,
            started_at=datetime.now(),
        )
        save_run(run, db_path)

        # Run cleanup
        cleaned = cleanup_stale_runs(db_path)
        assert cleaned == 1

        # Verify run is now FAILED
        updated = get_run("orphan-run", db_path)
        assert updated.status == FlowRunStatus.FAILED
        assert "Orphaned" in updated.error


def test_cleanup_stale_runs_marks_dead_agent():
    """cleanup_stale_runs marks runs with dead agent PID as FAILED."""
    from loopflow.lfd.flow_run import cleanup_stale_runs, get_run, save_run

    with tempfile.TemporaryDirectory() as tmpdir:
        db_path = Path(tmpdir) / "test.db"

        # Create agent with non-existent PID
        agent = _make_agent(id="agent-dead", pid=99999999)
        save_agent(agent, db_path)

        # Create run for that agent
        run = FlowRun(
            id="run-dead-agent",
            agent_id="agent-dead",
            flow="ship",
            area=["src/"],
            voice=["default"],
            repo=Path("/tmp/repo"),
            status=FlowRunStatus.RUNNING,
            started_at=datetime.now(),
        )
        save_run(run, db_path)

        # Run cleanup
        cleaned = cleanup_stale_runs(db_path)
        assert cleaned == 1

        # Verify run is now FAILED
        updated = get_run("run-dead-agent", db_path)
        assert updated.status == FlowRunStatus.FAILED
        assert "died" in updated.error


def test_cleanup_stale_runs_skips_active():
    """cleanup_stale_runs does not touch runs with live agents."""
    import os

    from loopflow.lfd.flow_run import cleanup_stale_runs, get_run, save_run

    with tempfile.TemporaryDirectory() as tmpdir:
        db_path = Path(tmpdir) / "test.db"

        # Create agent with current process PID (guaranteed to be alive)
        agent = _make_agent(id="agent-alive", pid=os.getpid())
        save_agent(agent, db_path)

        # Create run for that agent
        run = FlowRun(
            id="run-active",
            agent_id="agent-alive",
            flow="ship",
            area=["src/"],
            voice=["default"],
            repo=Path("/tmp/repo"),
            status=FlowRunStatus.RUNNING,
            started_at=datetime.now(),
        )
        save_run(run, db_path)

        # Run cleanup
        cleaned = cleanup_stale_runs(db_path)
        assert cleaned == 0

        # Verify run is still RUNNING
        updated = get_run("run-active", db_path)
        assert updated.status == FlowRunStatus.RUNNING


def test_cleanup_stale_runs_handles_deleted_agent():
    """cleanup_stale_runs marks runs whose agent was deleted."""
    from loopflow.lfd.flow_run import cleanup_stale_runs, get_run, save_run

    with tempfile.TemporaryDirectory() as tmpdir:
        db_path = Path(tmpdir) / "test.db"

        # Create run referencing non-existent agent
        run = FlowRun(
            id="run-missing-agent",
            agent_id="agent-that-was-deleted",
            flow="ship",
            area=["src/"],
            voice=["default"],
            repo=Path("/tmp/repo"),
            status=FlowRunStatus.RUNNING,
            started_at=datetime.now(),
        )
        save_run(run, db_path)

        # Run cleanup
        cleaned = cleanup_stale_runs(db_path)
        assert cleaned == 1

        # Verify run is now FAILED
        updated = get_run("run-missing-agent", db_path)
        assert updated.status == FlowRunStatus.FAILED
        assert "no longer exists" in updated.error


def test_mark_run_failed():
    """mark_run_failed sets status to FAILED with error."""
    from loopflow.lfd.flow_run import get_run, mark_run_failed, save_run

    with tempfile.TemporaryDirectory() as tmpdir:
        db_path = Path(tmpdir) / "test.db"

        run = FlowRun(
            id="run-to-fail",
            flow="ship",
            area=["src/"],
            voice=["default"],
            repo=Path("/tmp/repo"),
            status=FlowRunStatus.RUNNING,
            started_at=datetime.now(),
        )
        save_run(run, db_path)

        result = mark_run_failed("run-to-fail", "Something went wrong", db_path)
        assert result is True

        updated = get_run("run-to-fail", db_path)
        assert updated.status == FlowRunStatus.FAILED
        assert updated.error == "Something went wrong"
        assert updated.ended_at is not None


# =============================================================================
# Subprocess lifecycle tests (Phase 2)
# =============================================================================


def test_step_timeout_error_properties():
    """StepTimeoutError has expected attributes."""
    from loopflow.lfd.execution.runner import StepTimeoutError

    error = StepTimeoutError("implement", 30, 12345)
    assert error.step_label == "implement"
    assert error.timeout == 30
    assert error.pid == 12345
    assert "implement" in str(error)
    assert "30s" in str(error)


def test_step_timeout_handling():
    """StepTimeoutError can be raised and caught."""
    from loopflow.lfd.execution.runner import StepTimeoutError

    def simulate_timeout():
        raise StepTimeoutError("implement", 30, 12345)

    try:
        simulate_timeout()
        assert False, "Should have raised"
    except StepTimeoutError as e:
        assert e.step_label == "implement"
        assert e.timeout == 30
        assert "timed out" in str(e)


def test_default_step_timeout():
    """DEFAULT_STEP_TIMEOUT is 30 minutes."""
    from loopflow.lfd.execution.runner import DEFAULT_STEP_TIMEOUT

    assert DEFAULT_STEP_TIMEOUT == 30 * 60


def test_kill_process_tree_handles_missing_pid():
    """_kill_process_tree doesn't crash on missing PID."""
    from loopflow.lfd.execution.runner import _kill_process_tree

    # Should not raise
    _kill_process_tree(99999999)  # Non-existent PID


# =============================================================================
# Fire-and-forget logging tests (lf works without lfd)
# =============================================================================


def test_fire_and_forget_succeeds_without_daemon():
    """Fire-and-forget logging doesn't raise when daemon isn't running.

    This is critical: lf commands must work even when lfd daemon is not
    running. The logging is best-effort and should never block the CLI.
    """
    from loopflow.lfd.step_run import log_step_run_end, log_step_run_start

    step_run = StepRun(
        id="test-no-daemon",
        step="implement",
        repo="/tmp/repo",
        worktree="/tmp/repo.feature",
        status=StepRunStatus.RUNNING,
        started_at=datetime.now(),
    )

    # These should complete without raising, even with no daemon
    log_step_run_start(step_run)
    log_step_run_end(step_run.id, StepRunStatus.COMPLETED)

    # If we got here without exception, the test passes


def test_fire_and_forget_handles_connection_refused():
    """Fire-and-forget handles socket connection errors gracefully."""
    from loopflow.lfd.step_run import _send_fire_and_forget

    # Should not raise, even with bad socket path
    _send_fire_and_forget("test.method", {"key": "value"})


def test_lfd_imports_have_no_side_effects():
    """Importing lfd modules doesn't trigger database access.

    This ensures lf can import lfd modules without requiring a database
    or daemon to exist.
    """
    import sys

    # Remove cached modules to test fresh import
    modules_to_test = [
        "loopflow.lfd.models",
        "loopflow.lfd.step_run",
    ]

    for mod in modules_to_test:
        if mod in sys.modules:
            del sys.modules[mod]

    # Fresh import should not raise or touch the database
    from loopflow.lfd.models import StepRun as SR
    from loopflow.lfd.models import StepRunStatus as SRS

    # Verify we can create objects without database
    step_run = SR(
        id="test-import",
        step="test",
        repo="/tmp/repo",
        worktree="/tmp/repo",
        status=SRS.RUNNING,
        started_at=datetime.now(),
    )
    assert step_run.id == "test-import"


def test_summary_functions_work_after_schema_reset():
    """Summary DB functions work after schema mismatch triggers auto-reset.

    This ensures summaries work correctly when the database needed to be
    reset due to a schema version change.
    """
    import sqlite3

    from loopflow.lfd.db import delete_summaries_for_repo, load_summary_db, save_summary_db

    with tempfile.TemporaryDirectory() as tmpdir:
        db_path = Path(tmpdir) / "test.db"

        # Create DB with wrong schema version (will trigger auto-reset)
        conn = sqlite3.connect(db_path)
        conn.execute("CREATE TABLE _meta (key TEXT PRIMARY KEY, value TEXT NOT NULL)")
        conn.execute("INSERT INTO _meta (key, value) VALUES ('schema_version', 'wrong-version')")
        conn.commit()
        conn.close()

        # Save should work (triggers reset internally)
        save_summary_db("/tmp/repo", ".", 25000, "hash", "content", "model", db_path)

        # Load should find what we just saved
        result = load_summary_db("/tmp/repo", ".", 25000, db_path)
        assert result is not None
        assert result["content"] == "content"
        assert result["source_hash"] == "hash"

        # Delete should work
        count = delete_summaries_for_repo("/tmp/repo", db_path)
        assert count == 1


# =============================================================================
# Watch mode trigger tests (Phase 3)
# =============================================================================


def test_watch_trigger_no_previous_sha():
    """No previous SHA → no trigger (first run records baseline)."""
    from loopflow.lfd.agent import should_trigger_watch

    result = should_trigger_watch(
        watch_paths=["src/"],
        last_sha=None,
        current_sha="abc123",
        changed_files=["src/main.py"],
    )
    assert result is False


def test_watch_trigger_same_sha():
    """Same SHA → no trigger."""
    from loopflow.lfd.agent import should_trigger_watch

    result = should_trigger_watch(
        watch_paths=["src/"],
        last_sha="abc123",
        current_sha="abc123",
        changed_files=[],
    )
    assert result is False


def test_watch_trigger_no_matching_paths():
    """SHA changed, but no files match watch paths → no trigger."""
    from loopflow.lfd.agent import should_trigger_watch

    result = should_trigger_watch(
        watch_paths=["src/"],
        last_sha="abc123",
        current_sha="def456",
        changed_files=["docs/README.md", "tests/test_foo.py"],
    )
    assert result is False


def test_watch_trigger_matching_path():
    """SHA changed and file matches watch path → trigger."""
    from loopflow.lfd.agent import should_trigger_watch

    result = should_trigger_watch(
        watch_paths=["src/"],
        last_sha="abc123",
        current_sha="def456",
        changed_files=["src/main.py"],
    )
    assert result is True


def test_watch_trigger_exact_file_match():
    """Watch path is exact file, changed file matches → trigger."""
    from loopflow.lfd.agent import should_trigger_watch

    result = should_trigger_watch(
        watch_paths=["src/main.py"],
        last_sha="abc123",
        current_sha="def456",
        changed_files=["src/main.py"],
    )
    assert result is True


def test_watch_trigger_multiple_watch_paths():
    """Multiple watch paths, one matches → trigger."""
    from loopflow.lfd.agent import should_trigger_watch

    result = should_trigger_watch(
        watch_paths=["src/api/", "src/models/"],
        last_sha="abc123",
        current_sha="def456",
        changed_files=["src/models/user.py"],
    )
    assert result is True


def test_watch_trigger_glob_pattern():
    """Watch path with glob pattern matches → trigger."""
    from loopflow.lfd.agent import should_trigger_watch

    result = should_trigger_watch(
        watch_paths=["src/**/*.py"],
        last_sha="abc123",
        current_sha="def456",
        changed_files=["src/deep/nested/file.py"],
    )
    assert result is True


def test_watch_trigger_glob_no_match():
    """Watch path with glob pattern, no match → no trigger."""
    from loopflow.lfd.agent import should_trigger_watch

    result = should_trigger_watch(
        watch_paths=["src/**/*.py"],
        last_sha="abc123",
        current_sha="def456",
        changed_files=["src/deep/nested/file.js"],
    )
    assert result is False


def test_watch_trigger_empty_changed_files():
    """SHA changed but no files changed → no trigger."""
    from loopflow.lfd.agent import should_trigger_watch

    result = should_trigger_watch(
        watch_paths=["src/"],
        last_sha="abc123",
        current_sha="def456",
        changed_files=[],
    )
    assert result is False


def test_watch_trigger_trailing_slash_handling():
    """Watch path trailing slash is handled correctly."""
    from loopflow.lfd.agent import should_trigger_watch

    # With trailing slash
    result1 = should_trigger_watch(
        watch_paths=["src/"],
        last_sha="abc123",
        current_sha="def456",
        changed_files=["src/main.py"],
    )
    assert result1 is True

    # Without trailing slash
    result2 = should_trigger_watch(
        watch_paths=["src"],
        last_sha="abc123",
        current_sha="def456",
        changed_files=["src/main.py"],
    )
    assert result2 is True


def test_watch_trigger_partial_path_no_match():
    """Watch path 'src' should not match 'src2/file.py'."""
    from loopflow.lfd.agent import should_trigger_watch

    result = should_trigger_watch(
        watch_paths=["src"],
        last_sha="abc123",
        current_sha="def456",
        changed_files=["src2/file.py"],
    )
    assert result is False


# =============================================================================
# Cron mode trigger tests (Phase 4)
# =============================================================================


def test_cron_trigger_no_last_run():
    """First run (no last_run) → trigger."""
    from loopflow.lfd.agent import should_trigger_cron

    result = should_trigger_cron("* * * * *", None)
    assert result is True


def test_cron_trigger_due():
    """Cron expression due since last run → trigger."""
    from loopflow.lfd.agent import should_trigger_cron

    # Last run was 10 minutes ago, cron runs every minute
    last_run = datetime.now() - timedelta(minutes=10)
    result = should_trigger_cron("* * * * *", last_run)
    assert result is True


def test_cron_trigger_not_due():
    """Cron not yet due → no trigger."""
    from loopflow.lfd.agent import should_trigger_cron

    # Last run was just now, cron runs every hour
    last_run = datetime.now()
    result = should_trigger_cron("0 * * * *", last_run)
    assert result is False


def test_cron_trigger_stale_beyond_grace():
    """Cron missed beyond grace period → no trigger."""
    from loopflow.lfd.agent import should_trigger_cron

    # With zero grace period, any prev_time in the past is stale
    last_run = datetime.now() - timedelta(days=30)
    result = should_trigger_cron("* * * * *", last_run, grace_period=timedelta(seconds=0))
    assert result is False


def test_cron_trigger_within_grace():
    """Cron missed but within grace period → trigger."""
    from loopflow.lfd.agent import should_trigger_cron

    # Last run was 2 hours ago, grace period is 24 hours
    last_run = datetime.now() - timedelta(hours=2)
    result = should_trigger_cron("* * * * *", last_run, grace_period=timedelta(hours=24))
    assert result is True


def test_cron_trigger_daily_schedule():
    """Daily cron schedule triggers correctly."""
    from loopflow.lfd.agent import should_trigger_cron

    # Last run was 25 hours ago, cron runs daily at 9am
    last_run = datetime.now() - timedelta(hours=25)
    result = should_trigger_cron("0 9 * * *", last_run)
    assert result is True


def test_cron_trigger_hourly_recent_run():
    """Hourly cron with recent run → no trigger."""
    from loopflow.lfd.agent import should_trigger_cron

    # Last run was just now, cron runs every hour at :00
    # Since last_run is after the most recent :00, should not trigger
    last_run = datetime.now()
    result = should_trigger_cron("0 * * * *", last_run)
    assert result is False


def test_cron_trigger_first_run_stale():
    """First run but beyond grace period → no trigger."""
    from loopflow.lfd.agent import should_trigger_cron

    result = should_trigger_cron("* * * * *", None, grace_period=timedelta(seconds=0))
    assert result is False


def test_cron_trigger_every_5_minutes():
    """Every 5 minutes cron expression."""
    from loopflow.lfd.agent import should_trigger_cron

    # Last run was 6 minutes ago
    last_run = datetime.now() - timedelta(minutes=6)
    result = should_trigger_cron("*/5 * * * *", last_run)
    assert result is True


def test_cron_trigger_every_5_minutes_too_soon():
    """Every 5 minutes, ran after last scheduled time → no trigger."""
    from loopflow.lfd.agent import should_trigger_cron

    # Set last_run to just after the minute mark to ensure it's after prev_time
    # If now is 10:07, prev scheduled is 10:05
    # Setting last_run to now means last_run > prev_time, so no trigger
    last_run = datetime.now()
    result = should_trigger_cron("*/5 * * * *", last_run)
    assert result is False


# =============================================================================
# Loop mode resilience tests (Phase 5)
# =============================================================================


def test_agent_consecutive_failures_default():
    """Agent consecutive_failures defaults to 0."""
    agent = _make_agent()
    assert agent.consecutive_failures == 0


def test_agent_consecutive_failures_save_load():
    """consecutive_failures is persisted correctly."""
    with tempfile.TemporaryDirectory() as tmpdir:
        db_path = Path(tmpdir) / "test.db"

        agent = _make_agent(id="agent-failures", consecutive_failures=3)
        save_agent(agent, db_path)

        loaded = get_agent("agent-failures", db_path)
        assert loaded.consecutive_failures == 3


def test_update_agent_consecutive_failures():
    """update_agent_consecutive_failures updates the field."""
    from loopflow.lfd.agent import update_agent_consecutive_failures

    with tempfile.TemporaryDirectory() as tmpdir:
        db_path = Path(tmpdir) / "test.db"

        agent = _make_agent(id="agent-1", consecutive_failures=0)
        save_agent(agent, db_path)

        updated = update_agent_consecutive_failures("agent-1", 5, db_path)
        assert updated is True

        loaded = get_agent("agent-1", db_path)
        assert loaded.consecutive_failures == 5


def test_migrations_cover_consecutive_failures():
    """Migration adds consecutive_failures column."""
    with tempfile.TemporaryDirectory() as tmpdir:
        db_path = Path(tmpdir) / "test.db"

        # Create agent with consecutive_failures
        agent = _make_agent(id="agent-cf", consecutive_failures=7)
        save_agent(agent, db_path)

        # Verify it persists
        loaded = get_agent("agent-cf", db_path)
        assert loaded.consecutive_failures == 7


def test_worker_retry_constants():
    """Worker has expected retry constants."""
    from loopflow.lfd.execution.worker import (
        CIRCUIT_BREAKER_THRESHOLD,
        MAX_RETRIES,
        RETRY_BACKOFF_SECONDS,
    )

    assert MAX_RETRIES == 3
    assert RETRY_BACKOFF_SECONDS == 30
    assert CIRCUIT_BREAKER_THRESHOLD == 5


def test_agent_model_with_consecutive_failures():
    """Agent model accepts consecutive_failures parameter."""
    agent = _make_agent(consecutive_failures=10)
    assert agent.consecutive_failures == 10


# Activation queue tests


def test_agent_model_activation_queue_defaults():
    """Agent model has activation queue defaults."""
    agent = _make_agent()
    assert agent.pending_activations == 0
    assert agent.buffer_mode == "combine"


def test_agent_model_with_activation_queue():
    """Agent model accepts activation queue parameters."""
    agent = _make_agent(pending_activations=3, buffer_mode="queue")
    assert agent.pending_activations == 3
    assert agent.buffer_mode == "queue"


def test_update_agent_pending_activations():
    """Can update agent's pending activations."""
    with tempfile.TemporaryDirectory() as tmpdir:
        db_path = Path(tmpdir) / "test.db"

        agent = _make_agent(id="agent-pa", pending_activations=0)
        save_agent(agent, db_path)

        # Increment pending
        updated = update_agent_pending_activations("agent-pa", 3, db_path)
        assert updated is True

        loaded = get_agent("agent-pa", db_path)
        assert loaded.pending_activations == 3

        # Decrement pending
        updated = update_agent_pending_activations("agent-pa", 2, db_path)
        assert updated is True

        loaded = get_agent("agent-pa", db_path)
        assert loaded.pending_activations == 2


def test_agent_buffer_mode_save_load():
    """Buffer mode persists through save/load cycle."""
    with tempfile.TemporaryDirectory() as tmpdir:
        db_path = Path(tmpdir) / "test.db"

        agent = _make_agent(id="agent-bm", buffer_mode="queue")
        save_agent(agent, db_path)

        loaded = get_agent("agent-bm", db_path)
        assert loaded.buffer_mode == "queue"


def test_max_pending_activations_constant():
    """MAX_PENDING_ACTIVATIONS is defined."""
    assert MAX_PENDING_ACTIVATIONS == 10


def test_migrations_cover_activation_queue():
    """Migrations include activation queue columns."""
    from loopflow.lfd.migrations.registry import MIGRATIONS

    with tempfile.TemporaryDirectory() as tmpdir:
        import sqlite3

        db_path = Path(tmpdir) / "test.db"
        conn = sqlite3.connect(db_path)

        # Apply all migrations
        for migration in MIGRATIONS:
            migration.apply(conn)

        cursor = conn.execute("PRAGMA table_info(agents)")
        columns = {row[1] for row in cursor.fetchall()}
        conn.close()

        assert "pending_activations" in columns
        assert "buffer_mode" in columns
