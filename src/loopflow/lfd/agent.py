"""Agent entity persistence and operations."""

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
    Agent,
    AgentStatus,
    MergeMode,
    Stimulus,
    agent_from_row,
    area_to_slug,
)


def get_wt_from_cwd() -> Path | None:
    """Get the worktree path from current working directory."""
    return find_worktree_root()


# Persistence


def save_agent(agent: Agent, db_path: Path | None = None) -> None:
    """Save or update an agent."""
    conn = _get_db(db_path)

    conn.execute(
        """
        INSERT OR REPLACE INTO agents
        (id, repo, flow, voice, area, stimulus_kind, stimulus_cron, status, iteration,
         main_branch, pr_limit, merge_mode, pid, created_at, last_main_sha,
         consecutive_failures)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        """,
        (
            agent.id,
            str(agent.repo),
            agent.flow,
            json.dumps(agent.voice),
            json.dumps(agent.area),
            agent.stimulus.kind,
            agent.stimulus.cron,
            agent.status.value,
            agent.iteration,
            agent.main_branch,
            agent.pr_limit,
            agent.merge_mode.value,
            agent.pid,
            agent.created_at.isoformat(),
            agent.last_main_sha,
            agent.consecutive_failures,
        ),
    )
    conn.commit()
    conn.close()


def get_agent(agent_id: str, db_path: Path | None = None) -> Agent | None:
    """Get an agent by ID (supports short IDs)."""
    conn = _get_db(db_path)

    cursor = conn.execute("SELECT * FROM agents WHERE id = ?", (agent_id,))
    row = cursor.fetchone()

    if not row:
        cursor = conn.execute("SELECT * FROM agents WHERE id LIKE ?", (f"{agent_id}%",))
        row = cursor.fetchone()

    conn.close()
    return agent_from_row(dict(row)) if row else None


def get_agent_by_area_repo(
    area: list[str], repo: Path, db_path: Path | None = None
) -> Agent | None:
    """Get an agent by area and repo."""
    conn = _get_db(db_path)

    area_json = json.dumps(area)
    cursor = conn.execute(
        "SELECT * FROM agents WHERE area = ? AND repo = ?",
        (area_json, str(repo)),
    )
    row = cursor.fetchone()
    conn.close()
    return agent_from_row(dict(row)) if row else None


def list_agents(repo: Path | None = None, db_path: Path | None = None) -> list[Agent]:
    """List all agents, optionally filtered by repo."""
    conn = _get_db(db_path)

    if repo:
        cursor = conn.execute(
            "SELECT * FROM agents WHERE repo = ? ORDER BY created_at DESC",
            (str(repo),),
        )
    else:
        cursor = conn.execute("SELECT * FROM agents ORDER BY created_at DESC")

    agents = [agent_from_row(dict(row)) for row in cursor]
    conn.close()
    return agents


def update_agent_status(agent_id: str, status: AgentStatus, db_path: Path | None = None) -> bool:
    """Update an agent's status."""
    conn = _get_db(db_path)

    cursor = conn.execute(
        "UPDATE agents SET status = ? WHERE id = ? OR id LIKE ?",
        (status.value, agent_id, f"{agent_id}%"),
    )
    conn.commit()
    updated = cursor.rowcount > 0
    conn.close()
    return updated


def update_agent_iteration(agent_id: str, iteration: int, db_path: Path | None = None) -> bool:
    """Update an agent's iteration count."""
    conn = _get_db(db_path)

    cursor = conn.execute(
        "UPDATE agents SET iteration = ? WHERE id = ? OR id LIKE ?",
        (iteration, agent_id, f"{agent_id}%"),
    )
    conn.commit()
    updated = cursor.rowcount > 0
    conn.close()
    return updated


def update_agent_pid(agent_id: str, pid: int | None, db_path: Path | None = None) -> bool:
    """Update an agent's process ID."""
    conn = _get_db(db_path)

    cursor = conn.execute(
        "UPDATE agents SET pid = ? WHERE id = ? OR id LIKE ?",
        (pid, agent_id, f"{agent_id}%"),
    )
    conn.commit()
    updated = cursor.rowcount > 0
    conn.close()
    return updated


def update_agent_sha(agent_id: str, sha: str | None, db_path: Path | None = None) -> bool:
    """Update an agent's last_main_sha (for watch mode)."""
    conn = _get_db(db_path)

    cursor = conn.execute(
        "UPDATE agents SET last_main_sha = ? WHERE id = ? OR id LIKE ?",
        (sha, agent_id, f"{agent_id}%"),
    )
    conn.commit()
    updated = cursor.rowcount > 0
    conn.close()
    return updated


def update_agent_consecutive_failures(
    agent_id: str, failures: int, db_path: Path | None = None
) -> bool:
    """Update an agent's consecutive failure count."""
    conn = _get_db(db_path)

    cursor = conn.execute(
        "UPDATE agents SET consecutive_failures = ? WHERE id = ? OR id LIKE ?",
        (failures, agent_id, f"{agent_id}%"),
    )
    conn.commit()
    updated = cursor.rowcount > 0
    conn.close()
    return updated


def delete_agent(agent_id: str, db_path: Path | None = None) -> bool:
    """Delete an agent and its runs."""
    conn = _get_db(db_path)

    cursor = conn.execute(
        "SELECT id FROM agents WHERE id = ? OR id LIKE ?", (agent_id, f"{agent_id}%")
    )
    row = cursor.fetchone()
    if not row:
        conn.close()
        return False

    full_id = row["id"]

    conn.execute("DELETE FROM runs WHERE agent = ?", (full_id,))
    cursor = conn.execute("DELETE FROM agents WHERE id = ?", (full_id,))

    conn.commit()
    deleted = cursor.rowcount > 0
    conn.close()
    return deleted


# Branch management


def _allocate_main_branch(repo: Path, area: list[str]) -> str:
    """Allocate a unique branch name for an agent's main branch."""
    if area:
        slug = area_to_slug(area[0])
    else:
        slug = "root"

    for _ in range(100):
        words = generate_word_pair()
        candidate = f"{slug}-{words}-main"
        if not branch_exists(repo, candidate):
            return candidate

    raise ValueError(f"Could not allocate main branch for {slug}")


def _create_main_branch(repo: Path, branch: str) -> None:
    """Create main branch from origin/main if it doesn't exist."""
    if branch_exists(repo, branch):
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


def create_agent(
    repo: Path,
    flow: str,
    voice: list[str],
    area: list[str],
    pr_limit: int = 5,
    merge_mode: MergeMode = MergeMode.PR,
    stimulus: Stimulus | None = None,
) -> Agent:
    """Create or update an agent."""
    if stimulus is None:
        stimulus = Stimulus("loop")

    existing = get_agent_by_area_repo(area, repo)
    if existing:
        changed = False
        if set(existing.voice) != set(voice):
            existing.voice = voice
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
        if existing.stimulus.kind != stimulus.kind or existing.stimulus.cron != stimulus.cron:
            existing.stimulus = stimulus
            changed = True
        if changed:
            save_agent(existing)
        return existing

    main_branch = _allocate_main_branch(repo, area)
    _create_main_branch(repo, main_branch)

    agent = Agent(
        id=str(uuid.uuid4()),
        repo=repo,
        flow=flow,
        voice=voice,
        area=area,
        stimulus=stimulus,
        status=AgentStatus.IDLE,
        main_branch=main_branch,
        pr_limit=pr_limit,
        merge_mode=merge_mode,
    )

    save_agent(agent)
    return agent


def count_outstanding(agent: Agent) -> int:
    """Count commits on main_branch ahead of main."""
    subprocess.run(
        ["git", "fetch", "origin", "main", agent.main_branch],
        cwd=agent.repo,
        capture_output=True,
    )

    result = subprocess.run(
        ["git", "rev-list", "--count", f"origin/main..origin/{agent.main_branch}"],
        cwd=agent.repo,
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
    """Result of attempting to start an agent."""

    def __init__(self, ok: bool, reason: str | None = None, outstanding: int | None = None):
        self.ok = ok
        self.reason = reason
        self.outstanding = outstanding

    def __bool__(self) -> bool:
        return self.ok


def start_agent(agent_id: str, foreground: bool = False) -> StartResult:
    """Start an agent running."""
    from loopflow.lfd.daemon.process import is_process_running

    agent = get_agent(agent_id)
    if not agent:
        return StartResult(False, "not_found")

    if agent.status == AgentStatus.RUNNING and agent.pid and is_process_running(agent.pid):
        return StartResult(False, "already_running")

    outstanding = count_outstanding(agent)
    if outstanding >= agent.pr_limit:
        update_agent_status(agent_id, AgentStatus.WAITING)
        return StartResult(False, "waiting", outstanding=outstanding)

    if foreground:
        update_agent_status(agent_id, AgentStatus.RUNNING)
        update_agent_pid(agent_id, os.getpid())
        _run_agent(agent)
        return StartResult(True)
    else:
        proc = subprocess.Popen(
            [sys.executable, "-m", "loopflow.lfd.execution.worker", "agent", agent_id],
            cwd=agent.repo,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            start_new_session=True,
        )
        update_agent_status(agent_id, AgentStatus.RUNNING)
        update_agent_pid(agent_id, proc.pid)
        return StartResult(True)


def stop_agent(agent_id: str, force: bool = False) -> bool:
    """Stop a running agent."""
    from loopflow.lfd.daemon.process import is_process_running

    agent = get_agent(agent_id)
    if not agent:
        return False

    if agent.pid and is_process_running(agent.pid):
        sig = signal.SIGKILL if force else signal.SIGTERM
        try:
            os.kill(agent.pid, sig)
        except OSError:
            pass

    update_agent_status(agent_id, AgentStatus.IDLE)
    update_agent_pid(agent_id, None)
    return True


def _run_agent(agent: Agent) -> None:
    """Run the agent execution until it should pause."""
    from loopflow.lfd.execution.worker import run_agent_iterations

    run_agent_iterations(agent)


# Watch stimulus checking


def should_activate_watch(
    watch_paths: list[str],
    last_sha: str | None,
    current_sha: str,
    changed_files: list[str],
) -> bool:
    """Pure activation logic for watch stimulus.

    Returns True if agent should activate based on:
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


def check_watch_stimulus(agent: Agent) -> bool:
    """Check if watch-stimulus agent should run. Returns True if activated."""
    if agent.stimulus.kind != "watch":
        return False

    repo = agent.repo
    short_id = agent.short_id()
    # Use area as watch paths
    watch_paths = agent.area

    stimulus_log.debug(f"[{short_id}] watch check: area={agent.area_display}")

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
        f"[{short_id}] SHA: last={agent.last_main_sha[:7] if agent.last_main_sha else 'None'} "
        f"current={current_sha[:7]}"
    )

    if current_sha == agent.last_main_sha:
        return False

    if agent.last_main_sha is None:
        stimulus_log.info(f"[{short_id}] first check, recording baseline SHA {current_sha[:7]}")
        update_agent_sha(agent.id, current_sha)
        return False

    result = subprocess.run(
        ["git", "diff", "--name-only", agent.last_main_sha, current_sha, "--"] + watch_paths,
        cwd=repo,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        stimulus_log.warning(f"[{short_id}] git diff failed")
        update_agent_sha(agent.id, current_sha)
        return False

    changed_files = [f for f in result.stdout.strip().split("\n") if f]

    activated = should_activate_watch(watch_paths, agent.last_main_sha, current_sha, changed_files)

    if activated:
        stimulus_log.info(f"[{short_id}] ACTIVATED: {len(changed_files)} files changed in area")
        for f in changed_files[:5]:
            stimulus_log.debug(f"[{short_id}]   changed: {f}")
        if len(changed_files) > 5:
            stimulus_log.debug(f"[{short_id}]   ... and {len(changed_files) - 5} more")
    else:
        stimulus_log.debug(f"[{short_id}] no matching changes")

    update_agent_sha(agent.id, current_sha)
    return activated


# Cron stimulus checking

SCHEDULE_GRACE_PERIOD = timedelta(hours=24)


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


def check_cron_stimulus(agent: Agent) -> bool:
    """Check if cron-stimulus agent should run. Returns True if activated."""
    if agent.stimulus.kind != "cron" or not agent.stimulus.cron:
        return False

    from loopflow.lfd.flow_run import get_latest_run_for_agent

    short_id = agent.short_id()
    stimulus_log.debug(f"[{short_id}] cron check: expr={agent.stimulus.cron}")

    last_run = get_latest_run_for_agent(agent.id)
    last_time = last_run.ended_at if last_run else None

    activated = should_activate_cron(agent.stimulus.cron, last_time)

    if activated:
        stimulus_log.info(
            f"[{short_id}] ACTIVATED: cron={agent.stimulus.cron} last_run={last_time}"
        )
    else:
        stimulus_log.debug(f"[{short_id}] not due: last_run={last_time}")

    return activated


# Daemon stimulus check functions


def run_watch_check() -> list[str]:
    """Check all watch-stimulus agents and activate as needed."""
    agents = list_agents()
    watch_agents = [
        a for a in agents if a.stimulus.kind == "watch" and a.status != AgentStatus.RUNNING
    ]
    stimulus_log.debug(f"watch check: {len(watch_agents)} agents to check")

    activated = []
    for agent in watch_agents:
        try:
            if check_watch_stimulus(agent):
                result = start_agent(agent.id)
                if result:
                    stimulus_log.info(f"[{agent.short_id()}] started from watch stimulus")
                    activated.append(agent.id)
                else:
                    stimulus_log.warning(
                        f"[{agent.short_id()}] watch activated but start failed: {result.reason}"
                    )
        except Exception as e:
            stimulus_log.error(f"[{agent.short_id()}] watch check error: {e}")

    return activated


def run_cron_check() -> list[str]:
    """Check all cron-stimulus agents and activate as needed."""
    agents = list_agents()
    cron_agents = [
        a for a in agents if a.stimulus.kind == "cron" and a.status != AgentStatus.RUNNING
    ]
    stimulus_log.debug(f"cron check: {len(cron_agents)} agents to check")

    activated = []
    for agent in cron_agents:
        try:
            if check_cron_stimulus(agent):
                result = start_agent(agent.id)
                if result:
                    stimulus_log.info(f"[{agent.short_id()}] started from cron stimulus")
                    activated.append(agent.id)
                else:
                    stimulus_log.warning(
                        f"[{agent.short_id()}] cron activated but start failed: {result.reason}"
                    )
        except Exception as e:
            stimulus_log.error(f"[{agent.short_id()}] cron check error: {e}")

    return activated
