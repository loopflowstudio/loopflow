"""Trigger evaluation for agent runs."""

import subprocess
from datetime import datetime
from pathlib import Path

from loopflow.lfd.cron import parse_cron, should_run_cron
from loopflow.lfd.models import AgentSpec, AgentRun, TriggerKind


def should_trigger(agent: AgentSpec, last_run: AgentRun | None) -> bool:
    """Check if an agent's trigger condition is met."""
    if agent.trigger.kind == TriggerKind.MANUAL:
        return False

    if agent.trigger.kind == TriggerKind.LOOP:
        # Loop trigger: run again immediately after previous run completes
        # Only trigger if not currently running (caller checks this)
        return True

    if agent.trigger.kind == TriggerKind.MAIN_CHANGED:
        last_sha = last_run.main_sha if last_run else None
        changed, _ = check_main_changed(agent.repo, last_sha)
        return changed

    if agent.trigger.kind == TriggerKind.INTERVAL:
        last_run_at = last_run.started_at if last_run else None
        return _interval_elapsed(agent.trigger.interval_seconds, last_run_at)

    if agent.trigger.kind == TriggerKind.CRON:
        if not agent.trigger.cron:
            return False
        try:
            schedule = parse_cron(agent.trigger.cron)
            last_run_at = last_run.started_at if last_run else None
            return should_run_cron(
                schedule,
                last_run_at,
                datetime.now(),
                agent.trigger.grace_minutes,
            )
        except ValueError:
            return False

    return False


def check_main_changed(repo: Path, last_sha: str | None) -> tuple[bool, str | None]:
    """Fetch and compare main branch SHA. Returns (changed, current_sha)."""
    if not repo.exists():
        return False, None

    # Fetch latest from origin
    try:
        subprocess.run(
            ["git", "fetch", "origin", "main"],
            cwd=repo,
            capture_output=True,
            check=False,
        )
    except Exception:
        return False, None

    current_sha = _get_main_sha(repo)
    if not current_sha:
        return False, None

    if last_sha is None:
        return True, current_sha

    return current_sha != last_sha, current_sha


def _get_main_sha(repo: Path) -> str | None:
    """Get the current SHA of origin/main."""
    try:
        result = subprocess.run(
            ["git", "rev-parse", "origin/main"],
            cwd=repo,
            capture_output=True,
            text=True,
            check=False,
        )
        if result.returncode == 0:
            return result.stdout.strip()
    except Exception:
        pass
    return None


def _interval_elapsed(interval_seconds: int | None, last_run_at: datetime | None) -> bool:
    """Check if the interval has elapsed since last run."""
    if not interval_seconds:
        return False

    if last_run_at is None:
        return True

    elapsed = (datetime.now() - last_run_at).total_seconds()
    return elapsed >= interval_seconds
