"""Wave entity persistence and operations."""

import fnmatch
import json
import os
import signal
import subprocess
import sys
import uuid
from datetime import datetime, timedelta
from pathlib import Path

from croniter import croniter

from loopflow.lf.context import find_worktree_root
from loopflow.lf.naming import branch_exists, generate_word_pair
from loopflow.lfd.db import _get_db
from loopflow.lfd.logging import stimulus_log
from loopflow.lfd.models import (
    MergeMode,
    Stimulus,
    Wave,
    WaveStatus,
    wave_from_row,
)


def get_wt_from_cwd() -> Path | None:
    """Get the worktree path from current working directory."""
    return find_worktree_root()


# Persistence


def save_wave(wave: Wave, db_path: Path | None = None) -> None:
    """Save or update a wave."""
    conn = _get_db(db_path)

    conn.execute(
        """
        INSERT OR REPLACE INTO waves
        (id, name, repo, flow, direction, area, stimulus_kind, stimulus_cron,
         paused, status, iteration, worktree, branch, pr_limit, merge_mode,
         pid, created_at, last_main_sha, consecutive_failures, pending_activations)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        """,
        (
            wave.id,
            wave.name,
            str(wave.repo),
            wave.flow,
            json.dumps(wave.direction) if wave.direction is not None else None,
            json.dumps(wave.area) if wave.area is not None else None,
            wave.stimulus.kind,
            wave.stimulus.cron,
            1 if wave.paused else 0,
            wave.status.value,
            wave.iteration,
            str(wave.worktree) if wave.worktree else None,
            wave.branch,
            wave.pr_limit,
            wave.merge_mode.value,
            wave.pid,
            wave.created_at.isoformat(),
            wave.last_main_sha,
            wave.consecutive_failures,
            wave.pending_activations,
        ),
    )
    conn.commit()
    conn.close()


def get_wave(wave_id: str, db_path: Path | None = None) -> Wave | None:
    """Get a wave by ID (supports short IDs)."""
    conn = _get_db(db_path)

    cursor = conn.execute("SELECT * FROM waves WHERE id = ?", (wave_id,))
    row = cursor.fetchone()

    if not row:
        cursor = conn.execute("SELECT * FROM waves WHERE id LIKE ?", (f"{wave_id}%",))
        row = cursor.fetchone()

    conn.close()
    return wave_from_row(dict(row)) if row else None


def get_wave_by_area_repo(area: list[str], repo: Path, db_path: Path | None = None) -> Wave | None:
    """Get a wave by area and repo."""
    conn = _get_db(db_path)

    area_json = json.dumps(area)
    cursor = conn.execute(
        "SELECT * FROM waves WHERE area = ? AND repo = ?",
        (area_json, str(repo)),
    )
    row = cursor.fetchone()
    conn.close()
    return wave_from_row(dict(row)) if row else None


def get_wave_by_name(
    name: str, repo: Path | None = None, db_path: Path | None = None
) -> Wave | None:
    """Get a wave by name, optionally filtered by repo."""
    conn = _get_db(db_path)

    if repo:
        cursor = conn.execute(
            "SELECT * FROM waves WHERE name = ? AND repo = ?",
            (name, str(repo)),
        )
    else:
        cursor = conn.execute("SELECT * FROM waves WHERE name = ?", (name,))

    row = cursor.fetchone()
    conn.close()
    return wave_from_row(dict(row)) if row else None


def list_waves(repo: Path | None = None, db_path: Path | None = None) -> list[Wave]:
    """List all waves, optionally filtered by repo."""
    conn = _get_db(db_path)

    if repo:
        cursor = conn.execute(
            "SELECT * FROM waves WHERE repo = ? ORDER BY created_at DESC",
            (str(repo),),
        )
    else:
        cursor = conn.execute("SELECT * FROM waves ORDER BY created_at DESC")

    waves = [wave_from_row(dict(row)) for row in cursor]
    conn.close()
    return waves


def update_wave_status(wave_id: str, status: WaveStatus, db_path: Path | None = None) -> bool:
    """Update a wave's status."""
    conn = _get_db(db_path)

    cursor = conn.execute(
        "UPDATE waves SET status = ? WHERE id = ? OR id LIKE ?",
        (status.value, wave_id, f"{wave_id}%"),
    )
    conn.commit()
    updated = cursor.rowcount > 0
    conn.close()
    return updated


def update_wave_iteration(wave_id: str, iteration: int, db_path: Path | None = None) -> bool:
    """Update a wave's iteration count."""
    conn = _get_db(db_path)

    cursor = conn.execute(
        "UPDATE waves SET iteration = ? WHERE id = ? OR id LIKE ?",
        (iteration, wave_id, f"{wave_id}%"),
    )
    conn.commit()
    updated = cursor.rowcount > 0
    conn.close()
    return updated


def update_wave_pid(wave_id: str, pid: int | None, db_path: Path | None = None) -> bool:
    """Update a wave's process ID."""
    conn = _get_db(db_path)

    cursor = conn.execute(
        "UPDATE waves SET pid = ? WHERE id = ? OR id LIKE ?",
        (pid, wave_id, f"{wave_id}%"),
    )
    conn.commit()
    updated = cursor.rowcount > 0
    conn.close()
    return updated


def update_wave_sha(wave_id: str, sha: str | None, db_path: Path | None = None) -> bool:
    """Update a wave's last_main_sha (for watch mode)."""
    conn = _get_db(db_path)

    cursor = conn.execute(
        "UPDATE waves SET last_main_sha = ? WHERE id = ? OR id LIKE ?",
        (sha, wave_id, f"{wave_id}%"),
    )
    conn.commit()
    updated = cursor.rowcount > 0
    conn.close()
    return updated


def update_wave_consecutive_failures(
    wave_id: str, failures: int, db_path: Path | None = None
) -> bool:
    """Update a wave's consecutive failure count."""
    conn = _get_db(db_path)

    cursor = conn.execute(
        "UPDATE waves SET consecutive_failures = ? WHERE id = ? OR id LIKE ?",
        (failures, wave_id, f"{wave_id}%"),
    )
    conn.commit()
    updated = cursor.rowcount > 0
    conn.close()
    return updated


def update_wave_worktree_branch(
    wave_id: str,
    worktree: Path | None,
    branch: str | None,
    db_path: Path | None = None,
) -> bool:
    """Update a wave's worktree path and current branch."""
    conn = _get_db(db_path)

    cursor = conn.execute(
        "UPDATE waves SET worktree = ?, branch = ? WHERE id = ? OR id LIKE ?",
        (str(worktree) if worktree else None, branch, wave_id, f"{wave_id}%"),
    )
    conn.commit()
    updated = cursor.rowcount > 0
    conn.close()
    return updated


def update_wave_pending_activations(
    wave_id: str, pending: int, db_path: Path | None = None
) -> bool:
    """Update a wave's pending activations count."""
    conn = _get_db(db_path)

    cursor = conn.execute(
        "UPDATE waves SET pending_activations = ? WHERE id = ? OR id LIKE ?",
        (pending, wave_id, f"{wave_id}%"),
    )
    conn.commit()
    updated = cursor.rowcount > 0
    conn.close()
    return updated


def delete_wave(wave_id: str, db_path: Path | None = None) -> bool:
    """Delete a wave and its runs."""
    conn = _get_db(db_path)

    cursor = conn.execute(
        "SELECT id FROM waves WHERE id = ? OR id LIKE ?", (wave_id, f"{wave_id}%")
    )
    row = cursor.fetchone()
    if not row:
        conn.close()
        return False

    full_id = row["id"]

    conn.execute("DELETE FROM runs WHERE wave = ?", (full_id,))
    cursor = conn.execute("DELETE FROM waves WHERE id = ?", (full_id,))

    conn.commit()
    deleted = cursor.rowcount > 0
    conn.close()
    return deleted


# Wave naming


def _generate_wave_name(repo: Path) -> str:
    """Generate a unique wave name using word pairs."""
    for _ in range(100):
        words = generate_word_pair()
        # Check that {name}.main branch doesn't exist
        main_branch = f"{words}.main"
        if not branch_exists(repo, main_branch):
            return words

    raise ValueError("Could not generate unique wave name")


# Operations


def create_wave(
    repo: Path,
    name: str | None = None,
    flow: str = "ship",
    direction: list[str] | None = None,
    area: list[str] | None = None,
    pr_limit: int = 5,
    merge_mode: MergeMode = MergeMode.PR,
    stimulus: Stimulus | None = None,
) -> Wave:
    """Create a new wave or get existing by name.

    If name is provided and a wave with that name exists in the repo,
    returns the existing wave without modification (use update_wave for changes).

    """
    if stimulus is None:
        stimulus = Stimulus("loop")

    # Check for existing wave by name
    if name:
        existing = get_wave_by_name(name, repo)
        if existing:
            return existing

    wave_name = name or _generate_wave_name(repo)

    wave = Wave(
        id=str(uuid.uuid4()),
        name=wave_name,
        repo=repo,
        flow=flow,
        direction=direction,
        area=area,
        stimulus=stimulus,
        status=WaveStatus.IDLE,
        pr_limit=pr_limit,
        merge_mode=merge_mode,
    )

    save_wave(wave)
    return wave


def update_wave(
    wave_id: str,
    area: list[str] | None = None,
    direction: list[str] | None = None,
    flow: str | None = None,
    stimulus: Stimulus | None = None,
    paused: bool | None = None,
    pr_limit: int | None = None,
    merge_mode: MergeMode | None = None,
    db_path: Path | None = None,
) -> Wave | None:
    """Update a wave's configuration. Returns updated wave or None if not found."""
    wave = get_wave(wave_id, db_path)
    if not wave:
        return None

    if area is not None:
        wave.area = area
    if direction is not None:
        wave.direction = direction
    if flow is not None:
        wave.flow = flow
    if stimulus is not None:
        wave.stimulus = stimulus
    if paused is not None:
        wave.paused = paused
    if pr_limit is not None:
        wave.pr_limit = pr_limit
    if merge_mode is not None:
        wave.merge_mode = merge_mode

    save_wave(wave, db_path)
    return wave


def clone_wave(wave_id: str, name: str | None = None, db_path: Path | None = None) -> Wave | None:
    """Clone a wave with a new ID and name. Returns new wave or None if source not found."""
    from loopflow.lf.naming import generate_name

    source = get_wave(wave_id, db_path)
    if not source:
        return None

    new_wave = Wave(
        id=str(uuid.uuid4()),
        name=name or generate_name(),
        repo=source.repo,
        flow=source.flow,
        direction=source.direction,
        area=source.area,
        stimulus=source.stimulus,
        paused=True,  # Clones start paused
        status=WaveStatus.IDLE,
        iteration=0,
        worktree=None,  # New wave gets fresh worktree
        branch=None,
        pr_limit=source.pr_limit,
        merge_mode=source.merge_mode,
    )

    save_wave(new_wave, db_path)
    return new_wave


def count_outstanding(wave: Wave) -> int:
    """Count commits on main_branch ahead of main."""
    subprocess.run(
        ["git", "fetch", "origin", "main", wave.main_branch],
        cwd=wave.repo,
        capture_output=True,
    )

    result = subprocess.run(
        ["git", "rev-list", "--count", f"origin/main..origin/{wave.main_branch}"],
        cwd=wave.repo,
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
    """Result of attempting to start a wave."""

    def __init__(self, ok: bool, reason: str | None = None, outstanding: int | None = None):
        self.ok = ok
        self.reason = reason
        self.outstanding = outstanding

    def __bool__(self) -> bool:
        return self.ok


def start_wave(
    wave_id: str,
    foreground: bool = False,
    *,
    area: list[str] | None = None,
    direction: list[str] | None = None,
    flow: str | None = None,
    stimulus: "Stimulus | None" = None,
) -> StartResult:
    """Start a wave running.

    Optional kwargs are one-time overrides for this run only.
    They don't modify the wave's persistent configuration.
    """
    from loopflow.lfd.daemon.process import is_process_running

    wave = get_wave(wave_id)
    if not wave:
        return StartResult(False, "not_found")

    if wave.status == WaveStatus.RUNNING and wave.pid and is_process_running(wave.pid):
        return StartResult(False, "already_running")

    outstanding = count_outstanding(wave)
    if outstanding >= wave.pr_limit:
        update_wave_status(wave_id, WaveStatus.WAITING)
        return StartResult(False, "waiting", outstanding=outstanding)

    # Build CLI args for overrides
    override_args = []
    if area is not None:
        override_args.extend(["--area", ",".join(area)])
    if direction is not None:
        override_args.extend(["--direction", ",".join(direction)])
    if flow is not None:
        override_args.extend(["--flow", flow])
    if stimulus is not None:
        override_args.extend(["--stimulus", stimulus.kind])
        if stimulus.cron:
            override_args.extend(["--cron", stimulus.cron])

    if foreground:
        update_wave_status(wave_id, WaveStatus.RUNNING)
        update_wave_pid(wave_id, os.getpid())
        # Apply overrides to wave copy for foreground run
        if area is not None:
            wave.area = area
        if direction is not None:
            wave.direction = direction
        if flow is not None:
            wave.flow = flow
        if stimulus is not None:
            wave.stimulus = stimulus
        _run_wave(wave)
        return StartResult(True)
    else:
        proc = subprocess.Popen(
            [sys.executable, "-m", "loopflow.lfd.execution.worker", "wave", wave_id]
            + override_args,
            cwd=wave.repo,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            start_new_session=True,
        )
        update_wave_status(wave_id, WaveStatus.RUNNING)
        update_wave_pid(wave_id, proc.pid)
        return StartResult(True)


def stop_wave(wave_id: str, force: bool = False) -> bool:
    """Stop a running wave."""
    from loopflow.lfd.daemon.process import is_process_running

    wave = get_wave(wave_id)
    if not wave:
        return False

    if wave.pid and is_process_running(wave.pid):
        sig = signal.SIGKILL if force else signal.SIGTERM
        try:
            os.kill(wave.pid, sig)
        except OSError:
            pass

    update_wave_status(wave_id, WaveStatus.IDLE)
    update_wave_pid(wave_id, None)
    return True


def _run_wave(wave: Wave) -> None:
    """Run the wave execution until it should pause."""
    from loopflow.lfd.execution.worker import run_wave_iterations

    run_wave_iterations(wave)


# Watch stimulus checking


def should_activate_watch(
    watch_paths: list[str],
    last_sha: str | None,
    current_sha: str,
    changed_files: list[str],
) -> bool:
    """Pure activation logic for watch stimulus.

    Returns True if wave should activate based on:
    - SHA changed from last_sha to current_sha
    - At least one changed file matches watch_paths (area)
    """
    if last_sha is None:
        return False

    if current_sha == last_sha:
        return False

    if not changed_files:
        return False

    for changed in changed_files:
        for pattern in watch_paths:
            pattern = pattern.rstrip("/")
            if changed == pattern or changed.startswith(pattern + "/"):
                return True
            if "*" in pattern:
                if fnmatch.fnmatch(changed, pattern):
                    return True

    return False


def check_watch_stimulus(wave: Wave) -> bool:
    """Check if watch-stimulus wave should run. Returns True if activated."""
    if wave.stimulus.kind != "watch":
        return False

    repo = wave.repo
    short_id = wave.short_id()
    # Use area as watch paths
    watch_paths = wave.area

    stimulus_log.debug(f"[{short_id}] watch check: area={wave.area_display}")

    result = subprocess.run(["git", "fetch", "origin", "main"], cwd=repo, capture_output=True)
    if result.returncode != 0:
        stimulus_log.warning(f"[{short_id}] git fetch failed: {result.stderr.decode()[:200]}")
        return False

    result = subprocess.run(
        ["git", "rev-parse", "origin/main"],
        cwd=repo,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        stimulus_log.warning(f"[{short_id}] git rev-parse failed")
        return False

    current_sha = result.stdout.strip()
    stimulus_log.debug(
        f"[{short_id}] SHA: last={wave.last_main_sha[:7] if wave.last_main_sha else 'None'} "
        f"current={current_sha[:7]}"
    )

    if current_sha == wave.last_main_sha:
        return False

    if wave.last_main_sha is None:
        stimulus_log.info(f"[{short_id}] first check, recording baseline SHA {current_sha[:7]}")
        update_wave_sha(wave.id, current_sha)
        return False

    result = subprocess.run(
        ["git", "diff", "--name-only", wave.last_main_sha, current_sha, "--"] + watch_paths,
        cwd=repo,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        stimulus_log.warning(f"[{short_id}] git diff failed")
        update_wave_sha(wave.id, current_sha)
        return False

    changed_files = [f for f in result.stdout.strip().split("\n") if f]

    activated = should_activate_watch(watch_paths, wave.last_main_sha, current_sha, changed_files)

    if activated:
        stimulus_log.info(f"[{short_id}] ACTIVATED: {len(changed_files)} files changed in area")
        for f in changed_files[:5]:
            stimulus_log.debug(f"[{short_id}]   changed: {f}")
        if len(changed_files) > 5:
            stimulus_log.debug(f"[{short_id}]   ... and {len(changed_files) - 5} more")
    else:
        stimulus_log.debug(f"[{short_id}] no matching changes")

    update_wave_sha(wave.id, current_sha)
    return activated


# Cron stimulus checking

SCHEDULE_GRACE_PERIOD = timedelta(hours=24)
MAX_PENDING_ACTIVATIONS = 10


def should_activate_cron(
    cron_expr: str,
    last_run: datetime | None,
    grace_period: timedelta = SCHEDULE_GRACE_PERIOD,
) -> bool:
    """Check if cron should activate based on last run time."""
    now = datetime.now()
    cron = croniter(cron_expr, now)

    prev_time = cron.get_prev(datetime)

    if now - prev_time > grace_period:
        return False

    if last_run is None:
        return True

    return prev_time > last_run


def check_cron_stimulus(wave: Wave) -> bool:
    """Check if cron-stimulus wave should run. Returns True if activated."""
    if wave.stimulus.kind != "cron" or not wave.stimulus.cron:
        return False

    from loopflow.lfd.flow_run import get_latest_run_for_wave

    short_id = wave.short_id()
    stimulus_log.debug(f"[{short_id}] cron check: expr={wave.stimulus.cron}")

    last_run = get_latest_run_for_wave(wave.id)
    last_time = last_run.ended_at if last_run else None

    activated = should_activate_cron(wave.stimulus.cron, last_time)

    if activated:
        stimulus_log.info(f"[{short_id}] ACTIVATED: cron={wave.stimulus.cron} last_run={last_time}")
    else:
        stimulus_log.debug(f"[{short_id}] not due: last_run={last_time}")

    return activated


# Daemon stimulus check functions


def _queue_activation(wave: Wave) -> bool:
    """Queue an activation for a busy wave. Returns True if queued.

    Uses combine semantics: only one pending activation at a time (idempotent).
    """
    if wave.pending_activations >= MAX_PENDING_ACTIVATIONS:
        stimulus_log.warning(
            f"[{wave.short_id()}] pending activations at max ({MAX_PENDING_ACTIVATIONS}), dropping"
        )
        return False

    if wave.pending_activations == 0:
        update_wave_pending_activations(wave.id, 1)
        stimulus_log.info(f"[{wave.short_id()}] queued activation")
        return True
    stimulus_log.debug(f"[{wave.short_id()}] already has pending activation")
    return False


def run_watch_check() -> list[str]:
    """Check all watch-stimulus waves and activate or queue as needed."""
    waves = list_waves()
    watch_waves = [w for w in waves if w.stimulus.kind == "watch"]
    stimulus_log.debug(f"watch check: {len(watch_waves)} waves to check")

    activated = []
    for wave in watch_waves:
        try:
            if check_watch_stimulus(wave):
                if wave.status in (WaveStatus.RUNNING, WaveStatus.WAITING):
                    # Wave is busy, queue the activation
                    _queue_activation(wave)
                else:
                    # Wave is idle, start it
                    result = start_wave(wave.id)
                    if result:
                        stimulus_log.info(f"[{wave.short_id()}] started from watch stimulus")
                        activated.append(wave.id)
                    else:
                        stimulus_log.warning(
                            f"[{wave.short_id()}] watch activated but start failed: {result.reason}"
                        )
        except Exception as e:
            stimulus_log.error(f"[{wave.short_id()}] watch check error: {e}")

    return activated


def run_cron_check() -> list[str]:
    """Check all cron-stimulus waves and activate or queue as needed."""
    waves = list_waves()
    cron_waves = [w for w in waves if w.stimulus.kind == "cron"]
    stimulus_log.debug(f"cron check: {len(cron_waves)} waves to check")

    activated = []
    for wave in cron_waves:
        try:
            if check_cron_stimulus(wave):
                if wave.status in (WaveStatus.RUNNING, WaveStatus.WAITING):
                    # Wave is busy, queue the activation
                    _queue_activation(wave)
                else:
                    # Wave is idle, start it
                    result = start_wave(wave.id)
                    if result:
                        stimulus_log.info(f"[{wave.short_id()}] started from cron stimulus")
                        activated.append(wave.id)
                    else:
                        stimulus_log.warning(
                            f"[{wave.short_id()}] cron activated but start failed: {result.reason}"
                        )
        except Exception as e:
            stimulus_log.error(f"[{wave.short_id()}] cron check error: {e}")

    return activated
