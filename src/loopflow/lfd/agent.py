"""Agent entity persistence and operations."""

import json
import os
import random
import signal
import subprocess
import sys
import uuid
from datetime import datetime, timedelta
from pathlib import Path

from croniter import croniter

from loopflow.lf.context import find_worktree_root
from loopflow.lfd.db import _get_db
from loopflow.lfd.models import Agent, AgentStatus, MergeMode, agent_from_row, area_to_slug


def get_wt_from_cwd() -> Path | None:
    """Get the worktree path from current working directory."""
    return find_worktree_root()


# Word lists for generating unique branch names

MAGICAL = [
    "aurora",
    "cascade",
    "crystal",
    "drift",
    "echo",
    "ember",
    "fern",
    "flume",
    "frost",
    "glade",
    "grove",
    "haze",
    "ivy",
    "jade",
    "luna",
    "mist",
    "nova",
    "opal",
    "petal",
    "prism",
    "rain",
    "ripple",
    "sage",
    "shade",
    "spark",
    "star",
    "stone",
    "storm",
    "tide",
    "vale",
    "wave",
    "wisp",
    "wren",
    "zephyr",
]

MUSICAL = [
    "allegro",
    "aria",
    "ballad",
    "cadence",
    "canon",
    "chord",
    "coda",
    "duet",
    "forte",
    "fugue",
    "harmony",
    "hymn",
    "lilt",
    "lyric",
    "melody",
    "motif",
    "opus",
    "prelude",
    "refrain",
    "rondo",
    "sonata",
    "tempo",
    "trill",
    "tune",
    "verse",
    "waltz",
]


# Persistence


def save_agent(agent: Agent, db_path: Path | None = None) -> None:
    """Save or update an agent."""
    conn = _get_db(db_path)

    conn.execute(
        """
        INSERT OR REPLACE INTO agents
        (id, repo, flow, voice, area, status, iteration, main_branch,
         pr_limit, merge_mode, pid, created_at, watch_paths, cron, last_main_sha)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        """,
        (
            agent.id,
            str(agent.repo),
            agent.flow,
            json.dumps(agent.voice),
            json.dumps(agent.area),
            agent.status.value,
            agent.iteration,
            agent.main_branch,
            agent.pr_limit,
            agent.merge_mode.value,
            agent.pid,
            agent.created_at.isoformat(),
            agent.watch_paths,
            agent.cron,
            agent.last_main_sha,
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


def _allocate_main_branch(repo: Path, area: list[str]) -> str:
    """Allocate a unique branch name for an agent's main branch."""
    if area:
        slug = area_to_slug(area[0])
    else:
        slug = "root"

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


def create_agent(
    repo: Path,
    flow: str,
    voice: list[str],
    area: list[str],
    pr_limit: int = 5,
    merge_mode: MergeMode = MergeMode.PR,
    watch_paths: str | None = None,
    cron: str | None = None,
) -> Agent:
    """Create or update an agent."""
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
        if existing.watch_paths != watch_paths:
            existing.watch_paths = watch_paths
            changed = True
        if existing.cron != cron:
            existing.cron = cron
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
        status=AgentStatus.IDLE,
        main_branch=main_branch,
        pr_limit=pr_limit,
        merge_mode=merge_mode,
        watch_paths=watch_paths,
        cron=cron,
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


# Watch mode checking


def check_watch(agent: Agent) -> bool:
    """Check if watch-mode agent should run. Returns True if triggered."""
    if not agent.watch_paths:
        return False

    repo = agent.repo

    subprocess.run(["git", "fetch", "origin", "main"], cwd=repo, capture_output=True)

    result = subprocess.run(
        ["git", "rev-parse", "origin/main"],
        cwd=repo,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        return False

    current_sha = result.stdout.strip()

    if current_sha == agent.last_main_sha:
        return False

    if agent.last_main_sha is None:
        update_agent_sha(agent.id, current_sha)
        return False

    paths = [p.strip() for p in agent.watch_paths.split(",")]
    result = subprocess.run(
        ["git", "diff", "--name-only", agent.last_main_sha, current_sha, "--"] + paths,
        cwd=repo,
        capture_output=True,
        text=True,
    )

    changed_files = result.stdout.strip()
    if not changed_files:
        update_agent_sha(agent.id, current_sha)
        return False

    update_agent_sha(agent.id, current_sha)
    return True


# Cron mode checking

SCHEDULE_GRACE_PERIOD = timedelta(hours=24)


def should_trigger_cron(
    cron_expr: str,
    last_run: datetime | None,
    grace_period: timedelta = SCHEDULE_GRACE_PERIOD,
) -> bool:
    """Check if cron should trigger based on last run time."""
    now = datetime.now()
    cron = croniter(cron_expr, now)

    prev_time = cron.get_prev(datetime)

    if now - prev_time > grace_period:
        return False

    if last_run is None:
        return True

    return prev_time > last_run


def check_cron(agent: Agent) -> bool:
    """Check if cron-mode agent should run. Returns True if should trigger."""
    if not agent.cron:
        return False

    from loopflow.lfd.run import get_latest_run_for_agent

    last_run = get_latest_run_for_agent(agent.id)
    last_time = last_run.ended_at if last_run else None

    return should_trigger_cron(agent.cron, last_time)


# Daemon check functions


def run_watch_check() -> list[str]:
    """Check all watch-mode agents and trigger as needed."""
    triggered = []
    for agent in list_agents():
        if agent.status == AgentStatus.RUNNING:
            continue
        if not agent.watch_paths:
            continue

        if check_watch(agent):
            result = start_agent(agent.id)
            if result:
                triggered.append(agent.id)

    return triggered


def run_cron_check() -> list[str]:
    """Check all cron-mode agents and trigger as needed."""
    triggered = []
    for agent in list_agents():
        if agent.status == AgentStatus.RUNNING:
            continue
        if not agent.cron:
            continue

        if check_cron(agent):
            result = start_agent(agent.id)
            if result:
                triggered.append(agent.id)

    return triggered
