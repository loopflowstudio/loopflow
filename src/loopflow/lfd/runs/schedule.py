"""Schedule entity persistence and operations.

A Schedule spawns Runs on a cron schedule.
Designed for laptop use: missed schedules within 24h still trigger on wake.
"""

import json
import subprocess
import sys
import uuid
from datetime import datetime, timedelta
from pathlib import Path

from croniter import croniter

from loopflow.lfd.db import _get_db
from loopflow.lfd.models import MergeMode, Schedule, TriggerStatus
from loopflow.lfd.runs.loop import (
    StartResult,
    _allocate_main_branch,
    _create_main_branch,
    count_outstanding,
)
from loopflow.lfd.runs.run import get_latest_run_for_trigger

# Grace period for missed schedules (laptop was asleep/off)
SCHEDULE_GRACE_PERIOD = timedelta(hours=24)


# Persistence


def save_schedule(schedule: Schedule, db_path: Path | None = None) -> None:
    """Save or update a schedule."""
    conn = _get_db(db_path)

    conn.execute(
        """
        INSERT OR REPLACE INTO schedules
        (id, flow, area, repo, goals, cron,
         status, iteration, main_branch, pr_limit, merge_mode, pid, created_at)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        """,
        (
            schedule.id,
            schedule.flow,
            schedule.area,
            str(schedule.repo),
            json.dumps(schedule.goals) if schedule.goals else None,
            schedule.cron,
            schedule.status.value,
            schedule.iteration,
            schedule.main_branch,
            schedule.pr_limit,
            schedule.merge_mode.value,
            schedule.pid,
            schedule.created_at.isoformat(),
        ),
    )
    conn.commit()
    conn.close()


def get_schedule(schedule_id: str, db_path: Path | None = None) -> Schedule | None:
    """Get a schedule by ID (supports short IDs)."""
    conn = _get_db(db_path)

    cursor = conn.execute("SELECT * FROM schedules WHERE id = ?", (schedule_id,))
    row = cursor.fetchone()

    if not row:
        cursor = conn.execute("SELECT * FROM schedules WHERE id LIKE ?", (f"{schedule_id}%",))
        row = cursor.fetchone()

    conn.close()
    return _schedule_from_row(dict(row)) if row else None


def get_schedule_by_area_repo(
    area: str, repo: Path, db_path: Path | None = None
) -> Schedule | None:
    """Get a schedule by area and repo."""
    conn = _get_db(db_path)

    cursor = conn.execute(
        "SELECT * FROM schedules WHERE area = ? AND repo = ?",
        (area, str(repo)),
    )
    row = cursor.fetchone()
    conn.close()
    return _schedule_from_row(dict(row)) if row else None


def list_schedules(repo: Path | None = None, db_path: Path | None = None) -> list[Schedule]:
    """List all schedules, optionally filtered by repo."""
    conn = _get_db(db_path)

    if repo:
        cursor = conn.execute(
            "SELECT * FROM schedules WHERE repo = ? ORDER BY created_at DESC",
            (str(repo),),
        )
    else:
        cursor = conn.execute("SELECT * FROM schedules ORDER BY created_at DESC")

    schedules = [_schedule_from_row(dict(row)) for row in cursor]
    conn.close()
    return schedules


def update_schedule_status(
    schedule_id: str, status: TriggerStatus, db_path: Path | None = None
) -> bool:
    """Update a schedule's status."""
    conn = _get_db(db_path)

    cursor = conn.execute(
        "UPDATE schedules SET status = ? WHERE id = ? OR id LIKE ?",
        (status.value, schedule_id, f"{schedule_id}%"),
    )
    conn.commit()
    updated = cursor.rowcount > 0
    conn.close()
    return updated


def delete_schedule(schedule_id: str, db_path: Path | None = None) -> bool:
    """Delete a schedule and its runs."""
    conn = _get_db(db_path)

    cursor = conn.execute(
        "SELECT id FROM schedules WHERE id = ? OR id LIKE ?", (schedule_id, f"{schedule_id}%")
    )
    row = cursor.fetchone()
    if not row:
        conn.close()
        return False

    full_id = row["id"]

    conn.execute("DELETE FROM runs WHERE parent = ?", (f"schedule:{full_id}",))
    cursor = conn.execute("DELETE FROM schedules WHERE id = ?", (full_id,))

    conn.commit()
    deleted = cursor.rowcount > 0
    conn.close()
    return deleted


def _schedule_from_row(row: dict) -> Schedule:
    """Convert database row to Schedule."""
    goals_str = row.get("goals")
    goals = json.loads(goals_str) if goals_str else []

    merge_mode_str = row.get("merge_mode", "pr")
    if merge_mode_str == "auto":
        merge_mode_str = "pr"

    return Schedule(
        id=row["id"],
        flow=row["flow"],
        area=row["area"],
        repo=Path(row["repo"]),
        goals=goals,
        cron=row.get("cron", ""),
        status=TriggerStatus(row["status"]),
        iteration=row.get("iteration", 0),
        main_branch=row.get("main_branch", ""),
        pr_limit=row.get("pr_limit", 5),
        merge_mode=MergeMode(merge_mode_str),
        pid=row.get("pid"),
        created_at=datetime.fromisoformat(row["created_at"]),
    )


# Operations


def create_schedule(
    area: str,
    repo: Path,
    flow: str,
    cron: str,
    goals: list[str] | None = None,
    pr_limit: int = 5,
    merge_mode: MergeMode = MergeMode.PR,
) -> Schedule:
    """Create or get an existing schedule for an area+repo combination."""
    goals = goals or []

    existing = get_schedule_by_area_repo(area, repo)
    if existing:
        changed = False
        if set(existing.goals) != set(goals):
            existing.goals = goals
            changed = True
        if existing.flow != flow:
            existing.flow = flow
            changed = True
        if existing.cron != cron:
            existing.cron = cron
            changed = True
        if existing.pr_limit != pr_limit:
            existing.pr_limit = pr_limit
            changed = True
        if existing.merge_mode != merge_mode:
            existing.merge_mode = merge_mode
            changed = True
        if changed:
            save_schedule(existing)
        return existing

    main_branch = _allocate_main_branch(repo, area)
    _create_main_branch(repo, main_branch)

    schedule = Schedule(
        id=str(uuid.uuid4()),
        flow=flow,
        area=area,
        repo=repo,
        goals=goals,
        cron=cron,
        status=TriggerStatus.IDLE,
        main_branch=main_branch,
        pr_limit=pr_limit,
        merge_mode=merge_mode,
    )

    save_schedule(schedule)
    return schedule


def start_schedule(schedule_id: str) -> StartResult:
    """Trigger a schedule to spawn a Run."""
    schedule = get_schedule(schedule_id)
    if not schedule:
        return StartResult(False, "not_found")

    if schedule.status == TriggerStatus.RUNNING:
        return StartResult(False, "already_running")

    outstanding = count_outstanding(schedule)
    if outstanding >= schedule.pr_limit:
        update_schedule_status(schedule_id, TriggerStatus.WAITING)
        return StartResult(False, "waiting", outstanding=outstanding)

    update_schedule_status(schedule_id, TriggerStatus.RUNNING)
    subprocess.Popen(
        [sys.executable, "-m", "loopflow.lfd.execution.worker", "schedule", schedule_id],
        cwd=schedule.repo,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        start_new_session=True,
    )
    return StartResult(True)


# Checking (called periodically by daemon)


def should_trigger_cron(
    cron_expr: str,
    last_run: datetime | None,
    grace_period: timedelta = SCHEDULE_GRACE_PERIOD,
) -> bool:
    """Check if cron should trigger based on last run time.

    Handles laptop use: if computer was off at 9am but wakes at 2pm,
    the 9am schedule still runs. But if computer was off for a week,
    stale schedules are skipped.
    """
    now = datetime.now()
    cron = croniter(cron_expr, now)

    prev_time = cron.get_prev(datetime)

    if now - prev_time > grace_period:
        return False

    if last_run is None:
        return True

    return prev_time > last_run


def check_schedule(schedule: Schedule) -> bool:
    """Check if schedule should trigger. Returns True if should trigger."""
    if not schedule.cron:
        return False

    last_run = get_latest_run_for_trigger("schedule", schedule.id)
    last_time = last_run.ended_at if last_run else None

    return should_trigger_cron(schedule.cron, last_time)


def run_schedule_check() -> list[str]:
    """Check all schedules and trigger as needed.

    Returns list of schedule IDs that were triggered.
    """
    triggered = []
    for schedule in list_schedules():
        if schedule.status == TriggerStatus.RUNNING:
            continue

        if check_schedule(schedule):
            result = start_schedule(schedule.id)
            if result:
                triggered.append(schedule.id)

    return triggered
