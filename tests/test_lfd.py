"""Tests for lfd daemon."""

import tempfile
from datetime import datetime, timedelta
from pathlib import Path

from loopflow.lfd.agent import (
    load_agents,
    load_agents_for_repo,
    load_agents_for_worktree,
    save_agent,
    update_agent_status,
)
from loopflow.lfd.daemon.protocol import Event, Request, error, success
from loopflow.lfd.db import _get_db
from loopflow.lfd.migrations.registry import MIGRATIONS
from loopflow.lfd.models import (
    Agent,
    AgentStatus,
    MergeMode,
    Stimulus,
    Wave,
    WaveRun,
    WaveRunStatus,
    WaveStatus,
)
from loopflow.lfd.wave import (
    delete_wave,
    get_wave,
    get_wave_by_area_repo,
    list_waves,
    save_wave,
    update_wave_iteration,
    update_wave_pid,
    update_wave_status,
)
from loopflow.lfd.wave_run import (
    get_latest_wave_run_for_wave,
    list_wave_runs_for_wave,
    save_wave_run,
    update_wave_run_pr,
    update_wave_run_status,
    update_wave_run_step,
)


def test_agent_serialization():
    agent = Agent(
        id="agent-1",
        step="implement",
        repo="/tmp/repo",
        worktree="/tmp/repo.feature",
        status=AgentStatus.RUNNING,
        started_at=datetime(2024, 1, 1, 12, 0, 0),
        model="claude-code",
        run_mode="auto",
    )
    data = agent.to_dict()
    restored = Agent.from_dict(data)
    assert restored.step == "implement"
    assert restored.status == AgentStatus.RUNNING


def test_protocol_request_parse():
    line = '{"method": "sessions.list", "params": {"name": "test"}}'
    request = Request.parse(line)
    assert request.method == "sessions.list"
    assert request.params == {"name": "test"}


def test_protocol_response_serialize():
    resp = success({"waves": 5}, id="req-1")
    serialized = resp.serialize()
    assert '"ok": true' in serialized
    assert '"result":' in serialized


def test_protocol_error_response():
    resp = error("Not found")
    serialized = resp.serialize()
    assert '"ok": false' in serialized
    assert '"error": "Not found"' in serialized


def test_protocol_event_serialize():
    event = Event("wave.started", {"name": "test", "pid": 1234})
    serialized = event.serialize()
    assert '"event": "wave.started"' in serialized


def test_db_save_and_load_agent():
    with tempfile.TemporaryDirectory() as tmpdir:
        db_path = Path(tmpdir) / "test.db"
        agent = Agent(
            id="agent-1",
            step="implement",
            repo="/tmp/repo",
            worktree="/tmp/repo.feature",
            status=AgentStatus.RUNNING,
            started_at=datetime.now(),
        )
        save_agent(agent, db_path)

        agents = load_agents(db_path=db_path)
        assert len(agents) == 1
        assert agents[0].step == "implement"


def test_db_records_migrations():
    with tempfile.TemporaryDirectory() as tmpdir:
        db_path = Path(tmpdir) / "test.db"
        conn = _get_db(db_path)
        rows = conn.execute("SELECT version, applied_at FROM schema_migrations").fetchall()
        conn.close()

        # Should have all migrations recorded
        assert len(rows) == len(MIGRATIONS)
        recorded_versions = {row[0] for row in rows}
        expected_versions = {m.version for m in MIGRATIONS}
        assert recorded_versions == expected_versions
        for row in rows:
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
            "INSERT INTO waves (id, name, repo, flow, direction, area, "
            "stimulus_kind, paused, status, iteration, pr_limit, merge_mode, "
            "created_at) VALUES ('test', 'test-wave', '/tmp', 'ship', '[]', "
            "'[]', 'loop', 0, 'idle', 0, 5, 'pr', '2024-01-01')"
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
        cursor = conn.execute("SELECT COUNT(*) FROM waves")
        assert cursor.fetchone()[0] == 0
        conn.close()


# =============================================================================
# Migration completeness tests
#
# These tests verify that migrations create columns for ALL model fields.
# If a field is added to a model without a corresponding migration, the
# INSERT will fail and these tests catch it.
# =============================================================================


def test_migrations_cover_all_wave_fields():
    """Migrations create columns for all Wave model fields.

    Note: stimulus and last_main_sha are now in separate stimuli table.
    """
    with tempfile.TemporaryDirectory() as tmpdir:
        db_path = Path(tmpdir) / "test.db"

        wave = Wave(
            id="test-all-fields",
            name="aurora-melody",
            flow="ship",
            area=["src/test/"],
            direction=["direction-a", "direction-b"],
            repo=Path("/tmp/repo"),
            status=WaveStatus.RUNNING,
            iteration=5,
            pr_limit=10,
            merge_mode=MergeMode.LAND,
            pid=12345,
        )

        save_wave(wave, db_path)
        loaded = get_wave("test-all-fields", db_path)

        assert loaded.id == wave.id
        assert loaded.name == wave.name
        assert loaded.flow == wave.flow
        assert loaded.area == wave.area
        assert loaded.direction == wave.direction
        assert loaded.repo == wave.repo
        assert loaded.status == wave.status
        assert loaded.iteration == wave.iteration
        assert loaded.main_branch == "aurora-melody.main"  # computed
        assert loaded.pr_limit == wave.pr_limit
        assert loaded.merge_mode == wave.merge_mode
        assert loaded.pid == wave.pid


def test_migrations_cover_all_wave_run_fields():
    """Migrations create columns for all WaveRun model fields."""
    with tempfile.TemporaryDirectory() as tmpdir:
        db_path = Path(tmpdir) / "test.db"

        wave_run = WaveRun(
            id="test-wave-run-all-fields",
            wave_id="wave-id",
            flow="ship",
            area=["src/test/"],
            direction=["direction-a", "direction-b"],
            repo=Path("/tmp/repo"),
            status=WaveRunStatus.RUNNING,
            iteration=3,
            worktree="/tmp/repo.worktree",
            branch="feature-branch",
            current_step="implement",
            error="Something went wrong",
            pr_url="https://github.com/user/repo/pull/42",
            started_at=datetime(2024, 1, 15, 10, 30, 0),
            ended_at=datetime(2024, 1, 15, 11, 45, 0),
        )

        save_wave_run(wave_run, db_path)
        wave_runs = list_wave_runs_for_wave("wave-id", db_path=db_path)
        loaded = wave_runs[0]

        assert loaded.id == wave_run.id
        assert loaded.wave_id == wave_run.wave_id
        assert loaded.flow == wave_run.flow
        assert loaded.area == wave_run.area
        assert loaded.direction == wave_run.direction
        assert loaded.repo == wave_run.repo
        assert loaded.status == wave_run.status
        assert loaded.iteration == wave_run.iteration
        assert loaded.worktree == wave_run.worktree
        assert loaded.branch == wave_run.branch
        assert loaded.current_step == wave_run.current_step
        assert loaded.error == wave_run.error
        assert loaded.pr_url == wave_run.pr_url
        assert loaded.started_at == wave_run.started_at
        assert loaded.ended_at == wave_run.ended_at


def test_migrations_cover_all_agent_fields():
    """Migrations create columns for all Agent model fields."""
    with tempfile.TemporaryDirectory() as tmpdir:
        db_path = Path(tmpdir) / "test.db"

        agent = Agent(
            id="test-agent-all-fields",
            step="implement",
            repo="/tmp/repo",
            worktree="/tmp/repo.feature",
            status=AgentStatus.RUNNING,
            started_at=datetime(2024, 1, 15, 10, 30, 0),
            ended_at=datetime(2024, 1, 15, 11, 45, 0),
            pid=54321,
            model="claude:opus",
            run_mode="interactive",
        )

        save_agent(agent, db_path)
        agents = load_agents(db_path=db_path)
        loaded = agents[0]

        assert loaded.id == agent.id
        assert loaded.step == agent.step
        assert loaded.repo == agent.repo
        assert loaded.worktree == agent.worktree
        assert loaded.status == agent.status
        assert loaded.started_at == agent.started_at
        assert loaded.ended_at == agent.ended_at
        assert loaded.pid == agent.pid
        assert loaded.model == agent.model
        assert loaded.run_mode == agent.run_mode


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


def test_db_load_agents_for_worktree():
    with tempfile.TemporaryDirectory() as tmpdir:
        db_path = Path(tmpdir) / "test.db"

        agent1 = Agent(
            id="agent-1",
            step="implement",
            repo="/tmp/repo",
            worktree="/tmp/repo.feature-a",
            status=AgentStatus.COMPLETED,
            started_at=datetime(2024, 1, 1, 12, 0, 0),
        )
        agent2 = Agent(
            id="agent-2",
            step="review",
            repo="/tmp/repo",
            worktree="/tmp/repo.feature-a",
            status=AgentStatus.COMPLETED,
            started_at=datetime(2024, 1, 2, 12, 0, 0),
        )
        agent3 = Agent(
            id="agent-3",
            step="implement",
            repo="/tmp/repo",
            worktree="/tmp/repo.feature-b",
            status=AgentStatus.COMPLETED,
            started_at=datetime.now(),
        )

        save_agent(agent1, db_path)
        save_agent(agent2, db_path)
        save_agent(agent3, db_path)

        agents = load_agents_for_worktree("/tmp/repo.feature-a", db_path=db_path)
        assert len(agents) == 2
        # Should be ordered by started_at DESC
        assert agents[0].id == "agent-2"
        assert agents[1].id == "agent-1"


def test_db_load_agents_for_repo():
    with tempfile.TemporaryDirectory() as tmpdir:
        db_path = Path(tmpdir) / "test.db"

        agent1 = Agent(
            id="agent-1",
            step="implement",
            repo="/tmp/repo-a",
            worktree="/tmp/repo-a.feature",
            status=AgentStatus.COMPLETED,
            started_at=datetime.now(),
        )
        agent2 = Agent(
            id="agent-2",
            step="review",
            repo="/tmp/repo-b",
            worktree="/tmp/repo-b.feature",
            status=AgentStatus.COMPLETED,
            started_at=datetime.now(),
        )

        save_agent(agent1, db_path)
        save_agent(agent2, db_path)

        agents = load_agents_for_repo("/tmp/repo-a", db_path=db_path)
        assert len(agents) == 1
        assert agents[0].id == "agent-1"


def test_db_update_agent_status():
    with tempfile.TemporaryDirectory() as tmpdir:
        db_path = Path(tmpdir) / "test.db"

        agent = Agent(
            id="agent-1",
            step="implement",
            repo="/tmp/repo",
            worktree="/tmp/repo.feature",
            status=AgentStatus.RUNNING,
            started_at=datetime.now(),
        )
        save_agent(agent, db_path)

        updated = update_agent_status("agent-1", AgentStatus.COMPLETED, db_path)
        assert updated is True

        agents = load_agents_for_worktree("/tmp/repo.feature", db_path=db_path)
        assert len(agents) == 1
        assert agents[0].status == AgentStatus.COMPLETED
        assert agents[0].ended_at is not None


def test_db_update_agent_status_nonexistent():
    with tempfile.TemporaryDirectory() as tmpdir:
        db_path = Path(tmpdir) / "test.db"
        # Initialize DB by saving and then checking update
        agent = Agent(
            id="agent-1",
            step="implement",
            repo="/tmp/repo",
            worktree="/tmp/repo.feature",
            status=AgentStatus.RUNNING,
            started_at=datetime.now(),
        )
        save_agent(agent, db_path)

        updated = update_agent_status("nonexistent", AgentStatus.COMPLETED, db_path)
        assert updated is False


# Server handler tests


def test_server_handle_output_line_returns_success():
    """output.line handler returns success for valid params."""
    import asyncio

    from loopflow.lfd.daemon.server import Server

    async def run_test():
        with tempfile.TemporaryDirectory() as tmpdir:
            socket_path = Path(tmpdir) / "test.sock"
            server = Server(socket_path)

            # Mock broadcast to prevent side effects
            server._broadcast = lambda e: asyncio.sleep(0)

            params = {"agent_id": "test-agent-123", "text": "→ Read: foo.py"}
            response = await server._handle_output_line(params)

            assert response.ok is True
            assert response.result == {}

    asyncio.run(run_test())


def test_server_handle_output_line_missing_agent_id():
    """output.line handler returns error for missing agent_id."""
    import asyncio

    from loopflow.lfd.daemon.server import Server

    async def run_test():
        with tempfile.TemporaryDirectory() as tmpdir:
            socket_path = Path(tmpdir) / "test.sock"
            server = Server(socket_path)

            params = {"text": "→ Read: foo.py"}
            response = await server._handle_output_line(params)

            assert response.ok is False
            assert "agent_id" in response.error

    asyncio.run(run_test())


def test_server_handle_output_line_missing_text():
    """output.line handler returns error for missing text."""
    import asyncio

    from loopflow.lfd.daemon.server import Server

    async def run_test():
        with tempfile.TemporaryDirectory() as tmpdir:
            socket_path = Path(tmpdir) / "test.sock"
            server = Server(socket_path)

            params = {"agent_id": "test-agent-123"}
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

            # Mock broadcast to prevent side effects
            server._broadcast = lambda e: asyncio.sleep(0)

            # Empty string should be allowed (it's a blank line in output)
            params = {"agent_id": "test-agent-123", "text": ""}
            response = await server._handle_output_line(params)

            assert response.ok is True

    asyncio.run(run_test())


# Wave model tests


def _make_wave(**kwargs) -> Wave:
    """Helper to create a Wave with test defaults.

    Note: Wave no longer has stimulus field - stimuli are separate entities.
    """
    # Remove any stimulus-related kwargs since Wave no longer has these
    kwargs.pop("stimulus", None)
    kwargs.pop("cron", None)
    kwargs.pop("last_main_sha", None)

    defaults = {
        "id": "test-id",
        "name": "test-wave",
        "flow": "ship",
        "area": ["src/test/"],
        "direction": ["default"],
        "repo": Path("/tmp/repo"),
    }
    defaults.update(kwargs)
    return Wave(**defaults)


def test_wave_model_defaults():
    """Wave model has correct defaults."""
    wave = Wave(
        id="wave-1",
        name="test-coverage",
        flow="ship",
        area=["src/test-coverage/"],
        direction=["default"],
        repo=Path("/tmp/repo"),
    )
    assert wave.flow == "ship"
    assert wave.status == WaveStatus.IDLE
    assert wave.iteration == 0
    assert wave.pr_limit == 5
    assert wave.merge_mode == MergeMode.PR
    assert wave.pid is None
    assert wave.main_branch == "test-coverage.main"


def test_wave_model_short_id():
    """Wave.short_id() returns first 7 chars."""
    wave = _make_wave(id="abcdef1234567890")
    assert wave.short_id() == "abcdef1"


def test_stimulus_model():
    """Stimulus model has correct structure.

    Note: Stimulus is now a separate entity with id, wave_id, etc.
    """
    stimulus = Stimulus(
        id="stim-123",
        wave_id="wave-456",
        kind="cron",
        cron="0 9 * * *",
    )
    assert stimulus.kind == "cron"
    assert stimulus.cron == "0 9 * * *"
    assert stimulus.wave_id == "wave-456"
    assert stimulus.enabled is True  # default
    assert str(stimulus) == "cron(0 9 * * *)"

    loop_stimulus = Stimulus(id="stim-loop", wave_id="wave-1", kind="loop")
    assert str(loop_stimulus) == "loop"


def test_wave_run_model():
    """WaveRun model stores execution data."""
    wave_run = WaveRun(
        id="wave-run-1",
        wave_id="wave-1",
        flow="ship",
        area=["src/test/"],
        direction=["default"],
        repo=Path("/tmp/repo"),
        status=WaveRunStatus.RUNNING,
        iteration=3,
        worktree="/tmp/repo.wt",
        current_step="implement",
        started_at=datetime.now(),
    )
    assert wave_run.iteration == 3
    assert wave_run.status == WaveRunStatus.RUNNING
    assert wave_run.current_step == "implement"
    assert wave_run.ended_at is None
    assert wave_run.error is None
    assert wave_run.pr_url is None


# Wave database tests


def test_db_save_and_get_wave():
    """Save and retrieve a wave."""
    with tempfile.TemporaryDirectory() as tmpdir:
        db_path = Path(tmpdir) / "test.db"
        wave = _make_wave(
            id="wave-123",
            name="test-coverage",
            area=["src/test-coverage/"],
            status=WaveStatus.IDLE,
            iteration=0,
            pr_limit=5,
        )
        save_wave(wave, db_path)

        loaded = get_wave("wave-123", db_path)
        assert loaded is not None
        assert loaded.id == "wave-123"
        assert loaded.area == ["src/test-coverage/"]
        assert loaded.name == "test-coverage"
        assert loaded.main_branch == "test-coverage.main"
        assert loaded.flow == "ship"


def test_db_get_wave_short_id():
    """Get wave by short ID prefix."""
    with tempfile.TemporaryDirectory() as tmpdir:
        db_path = Path(tmpdir) / "test.db"
        wave = _make_wave(id="abcdef1234567890")
        save_wave(wave, db_path)

        # Should find by prefix
        loaded = get_wave("abcdef1", db_path)
        assert loaded is not None
        assert loaded.id == "abcdef1234567890"


def test_db_get_wave_by_area_repo():
    """Get wave by area and repo."""
    with tempfile.TemporaryDirectory() as tmpdir:
        db_path = Path(tmpdir) / "test.db"
        wave = _make_wave(
            id="wave-1",
            name="api-aurora-melody",
            area=["src/api/"],
        )
        save_wave(wave, db_path)

        loaded = get_wave_by_area_repo(["src/api/"], Path("/tmp/repo"), db_path=db_path)
        assert loaded is not None
        assert loaded.id == "wave-1"

        # Different area should not match
        not_found = get_wave_by_area_repo(["src/other/"], Path("/tmp/repo"), db_path=db_path)
        assert not_found is None


def test_db_list_waves():
    """List all waves."""
    with tempfile.TemporaryDirectory() as tmpdir:
        db_path = Path(tmpdir) / "test.db"

        wave1 = _make_wave(
            id="wave-1",
            name="direction-a",
            area=["src/direction-a/"],
            repo=Path("/tmp/repo-a"),
        )
        wave2 = _make_wave(
            id="wave-2",
            name="direction-b",
            area=["src/direction-b/"],
            repo=Path("/tmp/repo-b"),
        )
        save_wave(wave1, db_path)
        save_wave(wave2, db_path)

        waves = list_waves(db_path=db_path)
        assert len(waves) == 2

        # Filter by repo
        waves = list_waves(repo=Path("/tmp/repo-a"), db_path=db_path)
        assert len(waves) == 1
        assert waves[0].area == ["src/direction-a/"]


def test_db_update_wave_status():
    """Update wave status."""
    with tempfile.TemporaryDirectory() as tmpdir:
        db_path = Path(tmpdir) / "test.db"
        wave = _make_wave(
            id="wave-1",
            status=WaveStatus.IDLE,
        )
        save_wave(wave, db_path)

        updated = update_wave_status("wave-1", WaveStatus.RUNNING, db_path)
        assert updated is True

        loaded = get_wave("wave-1", db_path)
        assert loaded.status == WaveStatus.RUNNING


def test_db_update_wave_iteration():
    """Update wave iteration count."""
    with tempfile.TemporaryDirectory() as tmpdir:
        db_path = Path(tmpdir) / "test.db"
        wave = _make_wave(
            id="wave-1",
            iteration=0,
        )
        save_wave(wave, db_path)

        updated = update_wave_iteration("wave-1", 5, db_path)
        assert updated is True

        loaded = get_wave("wave-1", db_path)
        assert loaded.iteration == 5


def test_db_delete_wave():
    """Delete wave and its wave_runs."""
    with tempfile.TemporaryDirectory() as tmpdir:
        db_path = Path(tmpdir) / "test.db"
        wave = _make_wave(
            id="wave-1",
            name="test-wave",
            flow="ship",
            area=["src/test/"],
            repo=Path("/tmp/repo"),
        )
        save_wave(wave, db_path)

        # Add a wave_run
        wave_run = WaveRun(
            id="wave-run-1",
            wave_id="wave-1",
            flow="ship",
            area=["src/test/"],
            direction=["default"],
            repo=Path("/tmp/repo"),
            iteration=1,
            status=WaveRunStatus.RUNNING,
            started_at=datetime.now(),
        )
        save_wave_run(wave_run, db_path)

        # Delete wave (should also delete wave_runs)
        deleted = delete_wave("wave-1", db_path)
        assert deleted is True

        assert get_wave("wave-1", db_path) is None
        assert list_wave_runs_for_wave("wave-1", db_path=db_path) == []


# Run database tests


def test_db_save_and_get_wave_runs():
    """Save and retrieve wave_runs."""
    with tempfile.TemporaryDirectory() as tmpdir:
        db_path = Path(tmpdir) / "test.db"

        # Create parent wave first
        wave = _make_wave(id="wave-1")
        save_wave(wave, db_path)

        wave_run1 = WaveRun(
            id="wave-run-1",
            wave_id="wave-1",
            flow="ship",
            area=["src/test/"],
            direction=["default"],
            repo=Path("/tmp/repo"),
            iteration=1,
            status=WaveRunStatus.COMPLETED,
            started_at=datetime(2024, 1, 1, 12, 0, 0),
            pr_url="https://github.com/user/repo/pull/1",
        )
        wave_run2 = WaveRun(
            id="wave-run-2",
            wave_id="wave-1",
            flow="ship",
            area=["src/test/"],
            direction=["default"],
            repo=Path("/tmp/repo"),
            iteration=2,
            status=WaveRunStatus.RUNNING,
            started_at=datetime(2024, 1, 2, 12, 0, 0),
        )
        save_wave_run(wave_run1, db_path)
        save_wave_run(wave_run2, db_path)

        wave_runs = list_wave_runs_for_wave("wave-1", db_path=db_path)
        assert len(wave_runs) == 2


def test_db_get_latest_wave_run_for_wave():
    """Get most recent wave_run for a wave."""
    with tempfile.TemporaryDirectory() as tmpdir:
        db_path = Path(tmpdir) / "test.db"

        wave = _make_wave(id="wave-1")
        save_wave(wave, db_path)

        wave_run1 = WaveRun(
            id="wave-run-1",
            wave_id="wave-1",
            flow="ship",
            area=["src/test/"],
            direction=["default"],
            repo=Path("/tmp/repo"),
            iteration=1,
            status=WaveRunStatus.COMPLETED,
            started_at=datetime(2024, 1, 1, 12, 0, 0),
        )
        wave_run2 = WaveRun(
            id="wave-run-2",
            wave_id="wave-1",
            flow="ship",
            area=["src/test/"],
            direction=["default"],
            repo=Path("/tmp/repo"),
            iteration=2,
            status=WaveRunStatus.RUNNING,
            started_at=datetime(2024, 1, 2, 12, 0, 0),
        )
        save_wave_run(wave_run1, db_path)
        save_wave_run(wave_run2, db_path)

        latest = get_latest_wave_run_for_wave("wave-1", db_path)
        assert latest is not None
        assert latest.id == "wave-run-2"


def test_db_update_wave_run_status():
    """Update wave_run status."""
    with tempfile.TemporaryDirectory() as tmpdir:
        db_path = Path(tmpdir) / "test.db"

        wave = _make_wave(id="wave-1")
        save_wave(wave, db_path)

        wave_run = WaveRun(
            id="wave-run-1",
            wave_id="wave-1",
            flow="ship",
            area=["src/test/"],
            direction=["default"],
            repo=Path("/tmp/repo"),
            iteration=1,
            status=WaveRunStatus.RUNNING,
            started_at=datetime.now(),
        )
        save_wave_run(wave_run, db_path)

        updated = update_wave_run_status("wave-run-1", WaveRunStatus.COMPLETED, db_path=db_path)
        assert updated is True

        wave_runs = list_wave_runs_for_wave("wave-1", db_path=db_path)
        assert wave_runs[0].status == WaveRunStatus.COMPLETED
        assert wave_runs[0].ended_at is not None


def test_db_update_wave_run_step():
    """Update wave_run's current step."""
    with tempfile.TemporaryDirectory() as tmpdir:
        db_path = Path(tmpdir) / "test.db"

        wave = _make_wave(id="wave-1")
        save_wave(wave, db_path)

        wave_run = WaveRun(
            id="wave-run-1",
            wave_id="wave-1",
            flow="ship",
            area=["src/test/"],
            direction=["default"],
            repo=Path("/tmp/repo"),
            iteration=1,
            status=WaveRunStatus.RUNNING,
            started_at=datetime.now(),
        )
        save_wave_run(wave_run, db_path)

        updated = update_wave_run_step("wave-run-1", "implement", db_path)
        assert updated is True

        wave_runs = list_wave_runs_for_wave("wave-1", db_path=db_path)
        assert wave_runs[0].current_step == "implement"


def test_db_update_wave_run_pr():
    """Update wave_run's PR URL."""
    with tempfile.TemporaryDirectory() as tmpdir:
        db_path = Path(tmpdir) / "test.db"

        wave = _make_wave(id="wave-1")
        save_wave(wave, db_path)

        wave_run = WaveRun(
            id="wave-run-1",
            wave_id="wave-1",
            flow="ship",
            area=["src/test/"],
            direction=["default"],
            repo=Path("/tmp/repo"),
            iteration=1,
            status=WaveRunStatus.RUNNING,
            started_at=datetime.now(),
        )
        save_wave_run(wave_run, db_path)

        updated = update_wave_run_pr("wave-run-1", "https://github.com/user/repo/pull/42", db_path)
        assert updated is True

        wave_runs = list_wave_runs_for_wave("wave-1", db_path=db_path)
        assert wave_runs[0].pr_url == "https://github.com/user/repo/pull/42"


def test_db_update_wave_pid():
    """Update wave's process ID."""
    with tempfile.TemporaryDirectory() as tmpdir:
        db_path = Path(tmpdir) / "test.db"

        wave = _make_wave(id="wave-1")
        save_wave(wave, db_path)

        # Set pid
        updated = update_wave_pid("wave-1", 12345, db_path)
        assert updated is True

        loaded = get_wave("wave-1", db_path)
        assert loaded.pid == 12345

        # Clear pid
        updated = update_wave_pid("wave-1", None, db_path)
        assert updated is True

        loaded = get_wave("wave-1", db_path)
        assert loaded.pid is None


def test_wave_model_with_pid():
    """Wave model stores pid correctly."""
    wave = _make_wave(id="wave-1", pid=12345)
    assert wave.pid == 12345


def test_db_save_wave_with_pid():
    """Save and load wave with pid."""
    with tempfile.TemporaryDirectory() as tmpdir:
        db_path = Path(tmpdir) / "test.db"

        wave = _make_wave(id="wave-1", pid=54321)
        save_wave(wave, db_path)

        loaded = get_wave("wave-1", db_path)
        assert loaded.pid == 54321


# StartResult tests


def test_start_result_truthy_when_ok():
    """StartResult is truthy when ok=True."""
    from loopflow.lfd.wave import StartResult

    result = StartResult(True)
    assert result.ok is True
    assert result  # truthy
    assert result.reason is None
    assert result.outstanding is None


def test_start_result_falsy_when_not_ok():
    """StartResult is falsy when ok=False."""
    from loopflow.lfd.wave import StartResult

    result = StartResult(False, "already_running")
    assert result.ok is False
    assert not result  # falsy
    assert result.reason == "already_running"


def test_start_result_with_outstanding():
    """StartResult includes outstanding count for waiting state."""
    from loopflow.lfd.wave import StartResult

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


# =============================================================================
# State lifecycle tests (Phase 1)
# =============================================================================


def test_cleanup_stale_wave_runs_marks_orphaned():
    """cleanup_stale_wave_runs marks wave_runs without waves as FAILED."""
    from loopflow.lfd.wave_run import cleanup_stale_wave_runs, get_wave_run, save_wave_run

    with tempfile.TemporaryDirectory() as tmpdir:
        db_path = Path(tmpdir) / "test.db"

        # Create a wave_run with no wave
        wave_run = WaveRun(
            id="orphan-wave-run",
            wave_id=None,
            flow="ship",
            area=["src/"],
            direction=["default"],
            repo=Path("/tmp/repo"),
            status=WaveRunStatus.RUNNING,
            started_at=datetime.now(),
        )
        save_wave_run(wave_run, db_path)

        # Run cleanup
        cleaned = cleanup_stale_wave_runs(db_path)
        assert cleaned == 1

        # Verify wave_run is now FAILED
        updated = get_wave_run("orphan-wave-run", db_path)
        assert updated.status == WaveRunStatus.FAILED
        assert "Orphaned" in updated.error


def test_cleanup_stale_wave_runs_marks_dead_wave():
    """cleanup_stale_wave_runs marks wave_runs with dead wave PID as FAILED."""
    from loopflow.lfd.wave_run import cleanup_stale_wave_runs, get_wave_run, save_wave_run

    with tempfile.TemporaryDirectory() as tmpdir:
        db_path = Path(tmpdir) / "test.db"

        # Create wave with non-existent PID
        wave = _make_wave(id="wave-dead", pid=99999999)
        save_wave(wave, db_path)

        # Create wave_run for that wave
        wave_run = WaveRun(
            id="wave-run-dead-wave",
            wave_id="wave-dead",
            flow="ship",
            area=["src/"],
            direction=["default"],
            repo=Path("/tmp/repo"),
            status=WaveRunStatus.RUNNING,
            started_at=datetime.now(),
        )
        save_wave_run(wave_run, db_path)

        # Run cleanup
        cleaned = cleanup_stale_wave_runs(db_path)
        assert cleaned == 1

        # Verify wave_run is now FAILED
        updated = get_wave_run("wave-run-dead-wave", db_path)
        assert updated.status == WaveRunStatus.FAILED
        assert "died" in updated.error


def test_cleanup_stale_wave_runs_skips_active():
    """cleanup_stale_wave_runs does not touch wave_runs with live waves."""
    import os

    from loopflow.lfd.wave_run import cleanup_stale_wave_runs, get_wave_run, save_wave_run

    with tempfile.TemporaryDirectory() as tmpdir:
        db_path = Path(tmpdir) / "test.db"

        # Create wave with current process PID (guaranteed to be alive)
        wave = _make_wave(id="wave-alive", pid=os.getpid())
        save_wave(wave, db_path)

        # Create wave_run for that wave
        wave_run = WaveRun(
            id="wave-run-active",
            wave_id="wave-alive",
            flow="ship",
            area=["src/"],
            direction=["default"],
            repo=Path("/tmp/repo"),
            status=WaveRunStatus.RUNNING,
            started_at=datetime.now(),
        )
        save_wave_run(wave_run, db_path)

        # Run cleanup
        cleaned = cleanup_stale_wave_runs(db_path)
        assert cleaned == 0

        # Verify wave_run is still RUNNING
        updated = get_wave_run("wave-run-active", db_path)
        assert updated.status == WaveRunStatus.RUNNING


def test_cleanup_stale_wave_runs_handles_deleted_wave():
    """cleanup_stale_wave_runs marks wave_runs whose wave was deleted."""
    from loopflow.lfd.wave_run import cleanup_stale_wave_runs, get_wave_run, save_wave_run

    with tempfile.TemporaryDirectory() as tmpdir:
        db_path = Path(tmpdir) / "test.db"

        # Create wave_run referencing non-existent wave
        wave_run = WaveRun(
            id="wave-run-missing-wave",
            wave_id="wave-that-was-deleted",
            flow="ship",
            area=["src/"],
            direction=["default"],
            repo=Path("/tmp/repo"),
            status=WaveRunStatus.RUNNING,
            started_at=datetime.now(),
        )
        save_wave_run(wave_run, db_path)

        # Run cleanup
        cleaned = cleanup_stale_wave_runs(db_path)
        assert cleaned == 1

        # Verify wave_run is now FAILED
        updated = get_wave_run("wave-run-missing-wave", db_path)
        assert updated.status == WaveRunStatus.FAILED
        assert "no longer exists" in updated.error


def test_mark_wave_run_failed():
    """mark_wave_run_failed sets status to FAILED with error."""
    from loopflow.lfd.wave_run import get_wave_run, mark_wave_run_failed, save_wave_run

    with tempfile.TemporaryDirectory() as tmpdir:
        db_path = Path(tmpdir) / "test.db"

        wave_run = WaveRun(
            id="wave-run-to-fail",
            flow="ship",
            area=["src/"],
            direction=["default"],
            repo=Path("/tmp/repo"),
            status=WaveRunStatus.RUNNING,
            started_at=datetime.now(),
        )
        save_wave_run(wave_run, db_path)

        result = mark_wave_run_failed("wave-run-to-fail", "Something went wrong", db_path)
        assert result is True

        updated = get_wave_run("wave-run-to-fail", db_path)
        assert updated.status == WaveRunStatus.FAILED
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
    from loopflow.lfd.agent import log_agent_end, log_agent_start

    agent = Agent(
        id="test-no-daemon",
        step="implement",
        repo="/tmp/repo",
        worktree="/tmp/repo.feature",
        status=AgentStatus.RUNNING,
        started_at=datetime.now(),
    )

    # These should complete without raising, even with no daemon
    log_agent_start(agent)
    log_agent_end(agent.id, AgentStatus.COMPLETED)

    # If we got here without exception, the test passes


def test_fire_and_forget_handles_connection_refused():
    """Fire-and-forget handles socket connection errors gracefully."""
    from loopflow.lfd.agent import _send_fire_and_forget

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
        "loopflow.lfd.agent",
    ]

    for mod in modules_to_test:
        if mod in sys.modules:
            del sys.modules[mod]

    # Fresh import should not raise or touch the database
    from loopflow.lfd.models import Agent as A
    from loopflow.lfd.models import AgentStatus as AS

    # Verify we can create objects without database
    agent = A(
        id="test-import",
        step="test",
        repo="/tmp/repo",
        worktree="/tmp/repo",
        status=AS.RUNNING,
        started_at=datetime.now(),
    )
    assert agent.id == "test-import"


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


def test_watch_trigger_no_trigger_cases():
    """Watch mode should NOT trigger in these cases."""
    from loopflow.lfd.wave import should_activate_watch

    # No previous SHA (first run records baseline)
    assert should_activate_watch(["src/"], None, "abc123", ["src/main.py"]) is False

    # Same SHA (no change)
    assert should_activate_watch(["src/"], "abc123", "abc123", []) is False

    # No matching paths
    assert should_activate_watch(["src/"], "abc", "def", ["docs/README.md"]) is False

    # Empty changed files
    assert should_activate_watch(["src/"], "abc", "def", []) is False

    # Glob pattern no match
    assert should_activate_watch(["src/**/*.py"], "abc", "def", ["src/file.js"]) is False

    # Partial path shouldn't match (src vs src2)
    assert should_activate_watch(["src"], "abc", "def", ["src2/file.py"]) is False


def test_watch_trigger_should_trigger_cases():
    """Watch mode SHOULD trigger in these cases."""
    from loopflow.lfd.wave import should_activate_watch

    # Basic match
    assert should_activate_watch(["src/"], "abc", "def", ["src/main.py"]) is True

    # Exact file match
    assert should_activate_watch(["src/main.py"], "abc", "def", ["src/main.py"]) is True

    # Multiple watch paths, one matches
    assert (
        should_activate_watch(["src/api/", "src/models/"], "abc", "def", ["src/models/user.py"])
        is True
    )

    # Glob pattern match
    assert should_activate_watch(["src/**/*.py"], "abc", "def", ["src/deep/file.py"]) is True

    # Without trailing slash still works
    assert should_activate_watch(["src"], "abc", "def", ["src/main.py"]) is True


# =============================================================================
# Cron mode trigger tests (Phase 4)
# =============================================================================


def test_cron_trigger_no_trigger_cases():
    """Cron should NOT trigger in these cases."""
    from loopflow.lfd.wave import should_activate_cron

    now = datetime.now()

    # Just ran, hourly cron not due yet
    assert should_activate_cron("0 * * * *", now) is False

    # Beyond grace period (zero grace = never trigger stale)
    assert (
        should_activate_cron("* * * * *", now - timedelta(days=30), timedelta(seconds=0)) is False
    )

    # First run but beyond grace period
    assert should_activate_cron("* * * * *", None, timedelta(seconds=0)) is False

    # Just ran, 5-minute cron not due
    assert should_activate_cron("*/5 * * * *", now) is False


def test_cron_trigger_should_trigger_cases():
    """Cron SHOULD trigger in these cases."""
    from loopflow.lfd.wave import should_activate_cron

    now = datetime.now()

    # First run (no last_run) with default grace
    assert should_activate_cron("* * * * *", None) is True

    # Due (10 min since last run, runs every minute)
    assert should_activate_cron("* * * * *", now - timedelta(minutes=10)) is True

    # Within grace period
    assert should_activate_cron("* * * * *", now - timedelta(hours=2), timedelta(hours=24)) is True

    # Daily cron, 25 hours since last run
    assert should_activate_cron("0 9 * * *", now - timedelta(hours=25)) is True

    # 5-minute cron, 6 minutes since last run
    assert should_activate_cron("*/5 * * * *", now - timedelta(minutes=6)) is True


# =============================================================================
# Loop mode resilience tests (Phase 5)
# =============================================================================


def test_wave_consecutive_failures_default():
    """Wave consecutive_failures defaults to 0."""
    wave = _make_wave()
    assert wave.consecutive_failures == 0


def test_wave_consecutive_failures_save_load():
    """consecutive_failures is persisted correctly."""
    with tempfile.TemporaryDirectory() as tmpdir:
        db_path = Path(tmpdir) / "test.db"

        wave = _make_wave(id="wave-failures", consecutive_failures=3)
        save_wave(wave, db_path)

        loaded = get_wave("wave-failures", db_path)
        assert loaded.consecutive_failures == 3


def test_update_wave_consecutive_failures():
    """update_wave_consecutive_failures updates the field."""
    from loopflow.lfd.wave import update_wave_consecutive_failures

    with tempfile.TemporaryDirectory() as tmpdir:
        db_path = Path(tmpdir) / "test.db"

        wave = _make_wave(id="wave-1", consecutive_failures=0)
        save_wave(wave, db_path)

        updated = update_wave_consecutive_failures("wave-1", 5, db_path)
        assert updated is True

        loaded = get_wave("wave-1", db_path)
        assert loaded.consecutive_failures == 5


def test_migrations_cover_consecutive_failures():
    """Migration adds consecutive_failures column."""
    with tempfile.TemporaryDirectory() as tmpdir:
        db_path = Path(tmpdir) / "test.db"

        # Create wave with consecutive_failures
        wave = _make_wave(id="wave-cf", consecutive_failures=7)
        save_wave(wave, db_path)

        # Verify it persists
        loaded = get_wave("wave-cf", db_path)
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


def test_wave_model_with_consecutive_failures():
    """Wave model accepts consecutive_failures parameter."""
    wave = _make_wave(consecutive_failures=10)
    assert wave.consecutive_failures == 10


# =============================================================================
# Metrics tests
# =============================================================================


def test_metrics_increment():
    """Metrics increment and get_all work correctly."""
    from loopflow.lfd.daemon import metrics

    # Get initial values
    initial = metrics.get_all()

    # Increment
    metrics.increment("http_requests")
    metrics.increment("http_requests")
    metrics.increment("socket_requests", 5)

    # Check increments
    current = metrics.get_all()
    assert current["http_requests"] == initial["http_requests"] + 2
    assert current["socket_requests"] == initial["socket_requests"] + 5


def test_metrics_increment_unknown_counter():
    """Incrementing unknown counter is safe (no-op)."""
    from loopflow.lfd.daemon import metrics

    # Should not raise
    metrics.increment("nonexistent_counter")
    metrics.increment("also_nonexistent", 100)
