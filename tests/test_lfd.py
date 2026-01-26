"""Tests for lfd daemon."""

import tempfile
from datetime import datetime, timedelta
from pathlib import Path

from loopflow.lfd.daemon.protocol import Event, Request, error, success
from loopflow.lfd.db import _get_db
from loopflow.lfd.flow_run import (
    get_latest_run_for_wave,
    list_runs_for_wave,
    save_run,
    update_run_pr,
    update_run_status,
    update_run_step,
)
from loopflow.lfd.migrations.registry import MIGRATIONS
from loopflow.lfd.models import (
    FlowRun,
    FlowRunStatus,
    MergeMode,
    StepRun,
    StepRunStatus,
    Stimulus,
    Wave,
    WaveStatus,
)
from loopflow.lfd.step_run import (
    load_step_runs,
    load_step_runs_for_repo,
    load_step_runs_for_worktree,
    save_step_run,
    update_step_run_status,
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
    """Migrations create columns for all Wave model fields."""
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
            stimulus=Stimulus("cron", cron="0 9 * * *"),
            last_main_sha="abc123",
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
        assert loaded.stimulus.kind == wave.stimulus.kind
        assert loaded.stimulus.cron == wave.stimulus.cron
        assert loaded.last_main_sha == wave.last_main_sha


def test_migrations_cover_all_run_fields():
    """Migrations create columns for all Run model fields."""
    with tempfile.TemporaryDirectory() as tmpdir:
        db_path = Path(tmpdir) / "test.db"

        run = FlowRun(
            id="test-run-all-fields",
            wave_id="wave-id",
            flow="ship",
            area=["src/test/"],
            direction=["direction-a", "direction-b"],
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
        runs = list_runs_for_wave("wave-id", db_path=db_path)
        loaded = runs[0]

        assert loaded.id == run.id
        assert loaded.wave_id == run.wave_id
        assert loaded.flow == run.flow
        assert loaded.area == run.area
        assert loaded.direction == run.direction
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

            params = {"step_run_id": "test-step-run-123", "text": "→ Read: foo.py"}
            response = await server._handle_output_line(params)

            assert response.ok is True
            assert response.result == {}

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

            # Mock broadcast to prevent side effects
            server._broadcast = lambda e: asyncio.sleep(0)

            # Empty string should be allowed (it's a blank line in output)
            params = {"step_run_id": "test-step-run-123", "text": ""}
            response = await server._handle_output_line(params)

            assert response.ok is True

    asyncio.run(run_test())


# Wave model tests


def _make_wave(**kwargs) -> Wave:
    """Helper to create a Wave with test defaults."""
    # Determine stimulus from config if not explicitly set
    if "stimulus" not in kwargs:
        if kwargs.get("cron"):
            kwargs["stimulus"] = Stimulus("cron", cron=kwargs.pop("cron"))
        else:
            kwargs["stimulus"] = Stimulus("loop")
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


def test_wave_stimulus_property():
    """Wave.stimulus returns correct stimulus."""
    loop_wave = _make_wave()
    assert loop_wave.stimulus.kind == "loop"

    watch_wave = _make_wave(stimulus=Stimulus("watch"))
    assert watch_wave.stimulus.kind == "watch"

    cron_wave = _make_wave(stimulus=Stimulus("cron", cron="0 9 * * *"))
    assert cron_wave.stimulus.kind == "cron"
    assert cron_wave.stimulus.cron == "0 9 * * *"


def test_run_model():
    """Run model stores execution data."""
    run = FlowRun(
        id="run-1",
        wave_id="wave-1",
        flow="ship",
        area=["src/test/"],
        direction=["default"],
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
    """Delete wave and its runs."""
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

        # Add a run
        run = FlowRun(
            id="run-1",
            wave_id="wave-1",
            flow="ship",
            area=["src/test/"],
            direction=["default"],
            repo=Path("/tmp/repo"),
            iteration=1,
            status=FlowRunStatus.RUNNING,
            started_at=datetime.now(),
        )
        save_run(run, db_path)

        # Delete wave (should also delete runs)
        deleted = delete_wave("wave-1", db_path)
        assert deleted is True

        assert get_wave("wave-1", db_path) is None
        assert list_runs_for_wave("wave-1", db_path=db_path) == []


# Run database tests


def test_db_save_and_get_runs():
    """Save and retrieve runs."""
    with tempfile.TemporaryDirectory() as tmpdir:
        db_path = Path(tmpdir) / "test.db"

        # Create parent wave first
        wave = _make_wave(id="wave-1")
        save_wave(wave, db_path)

        run1 = FlowRun(
            id="run-1",
            wave_id="wave-1",
            flow="ship",
            area=["src/test/"],
            direction=["default"],
            repo=Path("/tmp/repo"),
            iteration=1,
            status=FlowRunStatus.COMPLETED,
            started_at=datetime(2024, 1, 1, 12, 0, 0),
            pr_url="https://github.com/user/repo/pull/1",
        )
        run2 = FlowRun(
            id="run-2",
            wave_id="wave-1",
            flow="ship",
            area=["src/test/"],
            direction=["default"],
            repo=Path("/tmp/repo"),
            iteration=2,
            status=FlowRunStatus.RUNNING,
            started_at=datetime(2024, 1, 2, 12, 0, 0),
        )
        save_run(run1, db_path)
        save_run(run2, db_path)

        runs = list_runs_for_wave("wave-1", db_path=db_path)
        assert len(runs) == 2


def test_db_get_latest_run_for_wave():
    """Get most recent run for a wave."""
    with tempfile.TemporaryDirectory() as tmpdir:
        db_path = Path(tmpdir) / "test.db"

        wave = _make_wave(id="wave-1")
        save_wave(wave, db_path)

        run1 = FlowRun(
            id="run-1",
            wave_id="wave-1",
            flow="ship",
            area=["src/test/"],
            direction=["default"],
            repo=Path("/tmp/repo"),
            iteration=1,
            status=FlowRunStatus.COMPLETED,
            started_at=datetime(2024, 1, 1, 12, 0, 0),
        )
        run2 = FlowRun(
            id="run-2",
            wave_id="wave-1",
            flow="ship",
            area=["src/test/"],
            direction=["default"],
            repo=Path("/tmp/repo"),
            iteration=2,
            status=FlowRunStatus.RUNNING,
            started_at=datetime(2024, 1, 2, 12, 0, 0),
        )
        save_run(run1, db_path)
        save_run(run2, db_path)

        latest = get_latest_run_for_wave("wave-1", db_path)
        assert latest is not None
        assert latest.id == "run-2"


def test_db_update_run_status():
    """Update run status."""
    with tempfile.TemporaryDirectory() as tmpdir:
        db_path = Path(tmpdir) / "test.db"

        wave = _make_wave(id="wave-1")
        save_wave(wave, db_path)

        run = FlowRun(
            id="run-1",
            wave_id="wave-1",
            flow="ship",
            area=["src/test/"],
            direction=["default"],
            repo=Path("/tmp/repo"),
            iteration=1,
            status=FlowRunStatus.RUNNING,
            started_at=datetime.now(),
        )
        save_run(run, db_path)

        updated = update_run_status("run-1", FlowRunStatus.COMPLETED, db_path=db_path)
        assert updated is True

        runs = list_runs_for_wave("wave-1", db_path=db_path)
        assert runs[0].status == FlowRunStatus.COMPLETED
        assert runs[0].ended_at is not None


def test_db_update_run_step():
    """Update run's current step."""
    with tempfile.TemporaryDirectory() as tmpdir:
        db_path = Path(tmpdir) / "test.db"

        wave = _make_wave(id="wave-1")
        save_wave(wave, db_path)

        run = FlowRun(
            id="run-1",
            wave_id="wave-1",
            flow="ship",
            area=["src/test/"],
            direction=["default"],
            repo=Path("/tmp/repo"),
            iteration=1,
            status=FlowRunStatus.RUNNING,
            started_at=datetime.now(),
        )
        save_run(run, db_path)

        updated = update_run_step("run-1", "implement", db_path)
        assert updated is True

        runs = list_runs_for_wave("wave-1", db_path=db_path)
        assert runs[0].current_step == "implement"


def test_db_update_run_pr():
    """Update run's PR URL."""
    with tempfile.TemporaryDirectory() as tmpdir:
        db_path = Path(tmpdir) / "test.db"

        wave = _make_wave(id="wave-1")
        save_wave(wave, db_path)

        run = FlowRun(
            id="run-1",
            wave_id="wave-1",
            flow="ship",
            area=["src/test/"],
            direction=["default"],
            repo=Path("/tmp/repo"),
            iteration=1,
            status=FlowRunStatus.RUNNING,
            started_at=datetime.now(),
        )
        save_run(run, db_path)

        updated = update_run_pr("run-1", "https://github.com/user/repo/pull/42", db_path)
        assert updated is True

        runs = list_runs_for_wave("wave-1", db_path=db_path)
        assert runs[0].pr_url == "https://github.com/user/repo/pull/42"


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


def test_cleanup_stale_runs_marks_orphaned():
    """cleanup_stale_runs marks runs without waves as FAILED."""
    from loopflow.lfd.flow_run import cleanup_stale_runs, get_run, save_run

    with tempfile.TemporaryDirectory() as tmpdir:
        db_path = Path(tmpdir) / "test.db"

        # Create a run with no wave
        run = FlowRun(
            id="orphan-run",
            wave_id=None,
            flow="ship",
            area=["src/"],
            direction=["default"],
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


def test_cleanup_stale_runs_marks_dead_wave():
    """cleanup_stale_runs marks runs with dead wave PID as FAILED."""
    from loopflow.lfd.flow_run import cleanup_stale_runs, get_run, save_run

    with tempfile.TemporaryDirectory() as tmpdir:
        db_path = Path(tmpdir) / "test.db"

        # Create wave with non-existent PID
        wave = _make_wave(id="wave-dead", pid=99999999)
        save_wave(wave, db_path)

        # Create run for that wave
        run = FlowRun(
            id="run-dead-wave",
            wave_id="wave-dead",
            flow="ship",
            area=["src/"],
            direction=["default"],
            repo=Path("/tmp/repo"),
            status=FlowRunStatus.RUNNING,
            started_at=datetime.now(),
        )
        save_run(run, db_path)

        # Run cleanup
        cleaned = cleanup_stale_runs(db_path)
        assert cleaned == 1

        # Verify run is now FAILED
        updated = get_run("run-dead-wave", db_path)
        assert updated.status == FlowRunStatus.FAILED
        assert "died" in updated.error


def test_cleanup_stale_runs_skips_active():
    """cleanup_stale_runs does not touch runs with live waves."""
    import os

    from loopflow.lfd.flow_run import cleanup_stale_runs, get_run, save_run

    with tempfile.TemporaryDirectory() as tmpdir:
        db_path = Path(tmpdir) / "test.db"

        # Create wave with current process PID (guaranteed to be alive)
        wave = _make_wave(id="wave-alive", pid=os.getpid())
        save_wave(wave, db_path)

        # Create run for that wave
        run = FlowRun(
            id="run-active",
            wave_id="wave-alive",
            flow="ship",
            area=["src/"],
            direction=["default"],
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


def test_cleanup_stale_runs_handles_deleted_wave():
    """cleanup_stale_runs marks runs whose wave was deleted."""
    from loopflow.lfd.flow_run import cleanup_stale_runs, get_run, save_run

    with tempfile.TemporaryDirectory() as tmpdir:
        db_path = Path(tmpdir) / "test.db"

        # Create run referencing non-existent wave
        run = FlowRun(
            id="run-missing-wave",
            wave_id="wave-that-was-deleted",
            flow="ship",
            area=["src/"],
            direction=["default"],
            repo=Path("/tmp/repo"),
            status=FlowRunStatus.RUNNING,
            started_at=datetime.now(),
        )
        save_run(run, db_path)

        # Run cleanup
        cleaned = cleanup_stale_runs(db_path)
        assert cleaned == 1

        # Verify run is now FAILED
        updated = get_run("run-missing-wave", db_path)
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
            direction=["default"],
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
