"""Loop entity persistence and operations.

A Loop continuously spawns Runs until stopped or paused (PR limit reached).
"""

import json
import os
import random
import signal
import subprocess
import sys
import uuid
from datetime import datetime
from pathlib import Path

from loopflow.lf.context import find_worktree_root
from loopflow.lfd.db import _get_db
from loopflow.lfd.models import Loop, MergeMode, Trigger, TriggerStatus, area_to_slug


def get_wt_from_cwd() -> Path | None:
    """Get the worktree path from current working directory."""
    return find_worktree_root()

# Word lists for generating unique branch names
MAGICAL = [
    "aurora", "cascade", "crystal", "drift", "echo", "ember", "fern", "flume",
    "frost", "glade", "grove", "haze", "ivy", "jade", "luna", "mist", "nova",
    "opal", "petal", "prism", "rain", "ripple", "sage", "shade", "spark",
    "star", "stone", "storm", "tide", "vale", "wave", "wisp", "wren", "zephyr",
]

MUSICAL = [
    "allegro", "aria", "ballad", "cadence", "canon", "chord", "coda", "duet",
    "forte", "fugue", "harmony", "hymn", "lilt", "lyric", "melody", "motif",
    "opus", "prelude", "refrain", "rondo", "sonata", "tempo", "trill", "tune",
    "verse", "waltz",
]


# Persistence


def save_loop(loop: Loop, db_path: Path | None = None) -> None:
    """Save or update a loop."""
    conn = _get_db(db_path)

    conn.execute(
        """
        INSERT OR REPLACE INTO loops
        (id, flow, area, repo, goals, status, iteration,
         main_branch, pr_limit, merge_mode, pid, created_at)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        """,
        (
            loop.id,
            loop.flow,
            loop.area,
            str(loop.repo),
            json.dumps(loop.goals) if loop.goals else None,
            loop.status.value,
            loop.iteration,
            loop.main_branch,
            loop.pr_limit,
            loop.merge_mode.value,
            loop.pid,
            loop.created_at.isoformat(),
        ),
    )
    conn.commit()
    conn.close()


def get_loop(loop_id: str, db_path: Path | None = None) -> Loop | None:
    """Get a loop by ID (supports short IDs)."""
    conn = _get_db(db_path)

    cursor = conn.execute("SELECT * FROM loops WHERE id = ?", (loop_id,))
    row = cursor.fetchone()

    if not row:
        cursor = conn.execute("SELECT * FROM loops WHERE id LIKE ?", (f"{loop_id}%",))
        row = cursor.fetchone()

    conn.close()
    return _loop_from_row(dict(row)) if row else None


def get_loop_by_area_repo(area: str, repo: Path, db_path: Path | None = None) -> Loop | None:
    """Get a loop by area and repo."""
    conn = _get_db(db_path)

    cursor = conn.execute(
        "SELECT * FROM loops WHERE area = ? AND repo = ?",
        (area, str(repo)),
    )
    row = cursor.fetchone()
    conn.close()
    return _loop_from_row(dict(row)) if row else None


def list_loops(repo: Path | None = None, db_path: Path | None = None) -> list[Loop]:
    """List all loops, optionally filtered by repo."""
    conn = _get_db(db_path)

    if repo:
        cursor = conn.execute(
            "SELECT * FROM loops WHERE repo = ? ORDER BY created_at DESC",
            (str(repo),),
        )
    else:
        cursor = conn.execute("SELECT * FROM loops ORDER BY created_at DESC")

    loops = [_loop_from_row(dict(row)) for row in cursor]
    conn.close()
    return loops


def update_loop_status(loop_id: str, status: TriggerStatus, db_path: Path | None = None) -> bool:
    """Update a loop's status."""
    conn = _get_db(db_path)

    cursor = conn.execute(
        "UPDATE loops SET status = ? WHERE id = ? OR id LIKE ?",
        (status.value, loop_id, f"{loop_id}%"),
    )
    conn.commit()
    updated = cursor.rowcount > 0
    conn.close()
    return updated


def update_loop_iteration(loop_id: str, iteration: int, db_path: Path | None = None) -> bool:
    """Update a loop's iteration count."""
    conn = _get_db(db_path)

    cursor = conn.execute(
        "UPDATE loops SET iteration = ? WHERE id = ? OR id LIKE ?",
        (iteration, loop_id, f"{loop_id}%"),
    )
    conn.commit()
    updated = cursor.rowcount > 0
    conn.close()
    return updated


def update_loop_pid(loop_id: str, pid: int | None, db_path: Path | None = None) -> bool:
    """Update a loop's process ID."""
    conn = _get_db(db_path)

    cursor = conn.execute(
        "UPDATE loops SET pid = ? WHERE id = ? OR id LIKE ?",
        (pid, loop_id, f"{loop_id}%"),
    )
    conn.commit()
    updated = cursor.rowcount > 0
    conn.close()
    return updated


def delete_loop(loop_id: str, db_path: Path | None = None) -> bool:
    """Delete a loop and its runs."""
    conn = _get_db(db_path)

    cursor = conn.execute(
        "SELECT id FROM loops WHERE id = ? OR id LIKE ?", (loop_id, f"{loop_id}%")
    )
    row = cursor.fetchone()
    if not row:
        conn.close()
        return False

    full_id = row["id"]

    conn.execute("DELETE FROM runs WHERE parent = ?", (f"loop:{full_id}",))
    cursor = conn.execute("DELETE FROM loops WHERE id = ?", (full_id,))

    conn.commit()
    deleted = cursor.rowcount > 0
    conn.close()
    return deleted


def _loop_from_row(row: dict) -> Loop:
    """Convert database row to Loop."""
    goals_str = row.get("goals")
    goals = json.loads(goals_str) if goals_str else []

    merge_mode_str = row.get("merge_mode", "pr")
    if merge_mode_str == "auto":
        merge_mode_str = "pr"

    return Loop(
        id=row["id"],
        flow=row["flow"],
        area=row["area"],
        repo=Path(row["repo"]),
        goals=goals,
        status=TriggerStatus(row["status"]),
        iteration=row.get("iteration", 0),
        main_branch=row.get("main_branch", ""),
        pr_limit=row.get("pr_limit", 5),
        merge_mode=MergeMode(merge_mode_str),
        pid=row.get("pid"),
        created_at=datetime.fromisoformat(row["created_at"]),
    )


# Branch management


def _generate_random_words() -> str:
    """Generate a random magical-musical pair like 'aurora-melody'."""
    magical = random.choice(MAGICAL)
    musical = random.choice(MUSICAL)
    return f"{magical}-{musical}"


def _branch_exists(repo: Path, branch: str) -> bool:
    """Check if a branch exists locally or on origin."""
    result = subprocess.run(
        ["git", "rev-parse", "--verify", f"refs/heads/{branch}"],
        cwd=repo,
        capture_output=True,
    )
    if result.returncode == 0:
        return True
    result = subprocess.run(
        ["git", "rev-parse", "--verify", f"refs/remotes/origin/{branch}"],
        cwd=repo,
        capture_output=True,
    )
    return result.returncode == 0


def _allocate_main_branch(repo: Path, area: str) -> str:
    """Allocate a unique branch name for a loop's main branch."""
    slug = area_to_slug(area)

    for _ in range(100):
        words = _generate_random_words()
        candidate = f"{slug}-{words}-main"
        if not _branch_exists(repo, candidate):
            return candidate

    raise ValueError(f"Could not allocate main branch for {slug}")


def _create_main_branch(repo: Path, branch: str) -> None:
    """Create main branch from origin/main if it doesn't exist."""
    if _branch_exists(repo, branch):
        return
    subprocess.run(["git", "fetch", "origin", "main"], cwd=repo, capture_output=True)
    result = subprocess.run(
        ["git", "branch", branch, "origin/main"],
        cwd=repo,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        subprocess.run(
            ["git", "branch", branch, "main"],
            cwd=repo,
            capture_output=True,
        )


# Operations


def create_loop(
    area: str,
    repo: Path,
    flow: str,
    goals: list[str] | None = None,
    pr_limit: int = 5,
    merge_mode: MergeMode = MergeMode.PR,
) -> Loop:
    """Create or get an existing loop for an area+repo combination."""
    goals = goals or []

    existing = get_loop_by_area_repo(area, repo)
    if existing:
        changed = False
        if set(existing.goals) != set(goals):
            existing.goals = goals
            changed = True
        if existing.flow != flow:
            existing.flow = flow
            changed = True
        if existing.pr_limit != pr_limit:
            existing.pr_limit = pr_limit
            changed = True
        if existing.merge_mode != merge_mode:
            existing.merge_mode = merge_mode
            changed = True
        if changed:
            save_loop(existing)
        return existing

    main_branch = _allocate_main_branch(repo, area)
    _create_main_branch(repo, main_branch)

    loop = Loop(
        id=str(uuid.uuid4()),
        flow=flow,
        area=area,
        repo=repo,
        goals=goals,
        status=TriggerStatus.IDLE,
        main_branch=main_branch,
        pr_limit=pr_limit,
        merge_mode=merge_mode,
    )

    save_loop(loop)
    return loop


def count_outstanding(trigger: Trigger) -> int:
    """Count commits on main_branch ahead of main."""
    subprocess.run(
        ["git", "fetch", "origin", "main", trigger.main_branch],
        cwd=trigger.repo,
        capture_output=True,
    )

    result = subprocess.run(
        ["git", "rev-list", "--count", f"origin/main..origin/{trigger.main_branch}"],
        cwd=trigger.repo,
        capture_output=True,
        text=True,
    )

    if result.returncode != 0:
        return 0

    try:
        return int(result.stdout.strip())
    except ValueError:
        return 0


class StartResult:
    """Result of attempting to start a trigger."""

    def __init__(self, ok: bool, reason: str | None = None, outstanding: int | None = None):
        self.ok = ok
        self.reason = reason
        self.outstanding = outstanding

    def __bool__(self) -> bool:
        return self.ok


def start_loop(loop_id: str, foreground: bool = False) -> StartResult:
    """Start a loop running."""
    from loopflow.lfd.daemon.process import is_process_running

    loop = get_loop(loop_id)
    if not loop:
        return StartResult(False, "not_found")

    if loop.status == TriggerStatus.RUNNING and loop.pid and is_process_running(loop.pid):
        return StartResult(False, "already_running")

    outstanding = count_outstanding(loop)
    if outstanding >= loop.pr_limit:
        update_loop_status(loop_id, TriggerStatus.WAITING)
        return StartResult(False, "waiting", outstanding=outstanding)

    if foreground:
        update_loop_status(loop_id, TriggerStatus.RUNNING)
        update_loop_pid(loop_id, os.getpid())
        _run_loop(loop)
        return StartResult(True)
    else:
        proc = subprocess.Popen(
            [sys.executable, "-m", "loopflow.lfd.execution.worker", "loop", loop_id],
            cwd=loop.repo,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            start_new_session=True,
        )
        update_loop_status(loop_id, TriggerStatus.RUNNING)
        update_loop_pid(loop_id, proc.pid)
        return StartResult(True)


def stop_loop(loop_id: str, force: bool = False) -> bool:
    """Stop a running loop."""
    from loopflow.lfd.daemon.process import is_process_running

    loop = get_loop(loop_id)
    if not loop:
        return False

    if loop.pid and is_process_running(loop.pid):
        sig = signal.SIGKILL if force else signal.SIGTERM
        try:
            os.kill(loop.pid, sig)
        except OSError:
            pass

    update_loop_status(loop_id, TriggerStatus.IDLE)
    update_loop_pid(loop_id, None)
    return True


def _run_loop(loop: Loop) -> None:
    """Run the loop execution until it should pause."""
    from loopflow.lfd.execution.worker import run_loop_iterations

    run_loop_iterations(loop)
