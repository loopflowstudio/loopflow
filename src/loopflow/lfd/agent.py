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
        (id, name, repo, flow, goal, area, stimulus_kind, stimulus_cron, status, iteration,
         worktree, branch, pr_limit, merge_mode, pid, created_at,
         last_main_sha, consecutive_failures, pending_activations)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        """,
        (
            agent.id,
            agent.name,
            str(agent.repo),
            agent.flow,
            json.dumps(agent.goal) if agent.goal is not None else None,
            json.dumps(agent.area) if agent.area is not None else None,
            agent.stimulus.kind,
            agent.stimulus.cron,
            agent.status.value,
            agent.iteration,
            str(agent.worktree) if agent.worktree else None,
            agent.branch,
            agent.pr_limit,
            agent.merge_mode.value,
            agent.pid,
            agent.created_at.isoformat(),
            agent.last_main_sha,
            agent.consecutive_failures,
            agent.pending_activations,
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


def get_agent_by_name(
    name: str, repo: Path | None = None, db_path: Path | None = None
) -> Agent | None:
    """Get an agent by name, optionally filtered by repo."""
    conn = _get_db(db_path)

    if repo:
        cursor = conn.execute(
            "SELECT * FROM agents WHERE name = ? AND repo = ?",
            (name, str(repo)),
        )
    else:
        cursor = conn.execute("SELECT * FROM agents WHERE name = ?", (name,))

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


def update_agent_worktree_branch(
    agent_id: str,
    worktree: Path | None,
    branch: str | None,
    db_path: Path | None = None,
) -> bool:
    """Update an agent's worktree path and current branch."""
    conn = _get_db(db_path)

    cursor = conn.execute(
        "UPDATE agents SET worktree = ?, branch = ? WHERE id = ? OR id LIKE ?",
        (str(worktree) if worktree else None, branch, agent_id, f"{agent_id}%"),
    )
    conn.commit()
    updated = cursor.rowcount > 0
    conn.close()
    return updated


def update_agent_pending_activations(
    agent_id: str, pending: int, db_path: Path | None = None
) -> bool:
    """Update an agent's pending activations count."""
    conn = _get_db(db_path)

    cursor = conn.execute(
        "UPDATE agents SET pending_activations = ? WHERE id = ? OR id LIKE ?",
        (pending, agent_id, f"{agent_id}%"),
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


# Agent naming


def _generate_agent_name(repo: Path) -> str:
    """Generate a unique agent name using word pairs."""
    for _ in range(100):
        words = generate_word_pair()
        # Check that {name}.main branch doesn't exist
        main_branch = f"{words}.main"
        if not branch_exists(repo, main_branch):
            return words

    raise ValueError("Could not generate unique agent name")


# Operations


def create_agent(
    repo: Path,
    name: str | None = None,
    flow: str = "ship",
    goal: list[str] | None = None,
    area: list[str] | None = None,
    pr_limit: int = 5,
    merge_mode: MergeMode = MergeMode.PR,
    stimulus: Stimulus | None = None,
) -> Agent:
    """Create a new agent or get existing by name.

    If name is provided and an agent with that name exists in the repo,
    returns the existing agent without modification (use update_agent for changes).

    """
    if stimulus is None:
        stimulus = Stimulus("loop")

    # Check for existing agent by name
    if name:
        existing = get_agent_by_name(name, repo)
        if existing:
            return existing

    agent_name = name or _generate_agent_name(repo)

    agent = Agent(
        id=str(uuid.uuid4()),
        name=agent_name,
        repo=repo,
        flow=flow,
        goal=goal,
        area=area,
        stimulus=stimulus,
        status=AgentStatus.IDLE,
        pr_limit=pr_limit,
        merge_mode=merge_mode,
    )

    save_agent(agent)
    return agent


def update_agent(
    agent_id: str,
    area: list[str] | None = None,
    goal: list[str] | None = None,
    flow: str | None = None,
    stimulus: Stimulus | None = None,
    pr_limit: int | None = None,
    merge_mode: MergeMode | None = None,
    db_path: Path | None = None,
) -> Agent | None:
    """Update an agent's configuration. Returns updated agent or None if not found."""
    agent = get_agent(agent_id, db_path)
    if not agent:
        return None

    if area is not None:
        agent.area = area
    if goal is not None:
        agent.goal = goal
    if flow is not None:
        agent.flow = flow
    if stimulus is not None:
        agent.stimulus = stimulus
    if pr_limit is not None:
        agent.pr_limit = pr_limit
    if merge_mode is not None:
        agent.merge_mode = merge_mode

    save_agent(agent, db_path)
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


def _queue_activation(agent: Agent) -> bool:
    """Queue an activation for a busy agent. Returns True if queued.

    Uses combine semantics: only one pending activation at a time (idempotent).
    """
    if agent.pending_activations >= MAX_PENDING_ACTIVATIONS:
        stimulus_log.warning(
            f"[{agent.short_id()}] pending activations at max ({MAX_PENDING_ACTIVATIONS}), dropping"
        )
        return False

    if agent.pending_activations == 0:
        update_agent_pending_activations(agent.id, 1)
        stimulus_log.info(f"[{agent.short_id()}] queued activation")
        return True
    stimulus_log.debug(f"[{agent.short_id()}] already has pending activation")
    return False


def run_watch_check() -> list[str]:
    """Check all watch-stimulus agents and activate or queue as needed."""
    agents = list_agents()
    watch_agents = [a for a in agents if a.stimulus.kind == "watch"]
    stimulus_log.debug(f"watch check: {len(watch_agents)} agents to check")

    activated = []
    for agent in watch_agents:
        try:
            if check_watch_stimulus(agent):
                if agent.status in (AgentStatus.RUNNING, AgentStatus.WAITING):
                    # Agent is busy, queue the activation
                    _queue_activation(agent)
                else:
                    # Agent is idle, start it
                    result = start_agent(agent.id)
                    if result:
                        stimulus_log.info(f"[{agent.short_id()}] started from watch stimulus")
                        activated.append(agent.id)
                    else:
                        stimulus_log.warning(
                            f"[{agent.short_id()}] watch activated but start failed: "
                            f"{result.reason}"
                        )
        except Exception as e:
            stimulus_log.error(f"[{agent.short_id()}] watch check error: {e}")

    return activated


def run_cron_check() -> list[str]:
    """Check all cron-stimulus agents and activate or queue as needed."""
    agents = list_agents()
    cron_agents = [a for a in agents if a.stimulus.kind == "cron"]
    stimulus_log.debug(f"cron check: {len(cron_agents)} agents to check")

    activated = []
    for agent in cron_agents:
        try:
            if check_cron_stimulus(agent):
                if agent.status in (AgentStatus.RUNNING, AgentStatus.WAITING):
                    # Agent is busy, queue the activation
                    _queue_activation(agent)
                else:
                    # Agent is idle, start it
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
