"""Trigger evaluation for agent runs.

Triggers determine when an agent should run:
- manual: Only runs when explicitly started
- loop: Runs again immediately after previous run completes
- main-changed: Runs when origin/main has new commits since last run
- interval: Runs every N seconds
- cron: Runs on a schedule (minute hour day month weekday)
"""

import subprocess
from datetime import datetime
from pathlib import Path

from loopflow.maestro.markdown import AgentFile


def should_trigger(
    agent: AgentFile,
    last_run_at: datetime | None,
    last_main_sha: str | None,
) -> bool:
    """Check if an agent's trigger condition is met."""
    if agent.trigger == "manual":
        return False

    if agent.trigger == "loop":
        # Loop trigger: run again immediately after previous run completes
        return True

    if agent.trigger == "cron":
        return _cron_matches(agent.cron, last_run_at)

    if agent.trigger == "main-changed":
        return _main_changed(agent.repo, last_main_sha)

    if agent.trigger == "interval":
        return _interval_elapsed(agent.interval_seconds, last_run_at)

    return False


def _main_changed(repo: Path, last_main_sha: str | None) -> bool:
    """Check if origin/main has new commits."""
    if not repo.exists():
        return False

    # Fetch latest from origin
    try:
        subprocess.run(
            ["git", "fetch", "origin", "main"],
            cwd=repo,
            capture_output=True,
            check=False,
        )
    except Exception:
        return False

    current_sha = _get_main_sha(repo)
    if not current_sha:
        return False

    if last_main_sha is None:
        return True

    return current_sha != last_main_sha


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


def get_main_sha(repo: Path) -> str | None:
    """Get the current SHA of origin/main. Public API for storing state."""
    return _get_main_sha(repo)


def _cron_matches(cron_expr: str | None, last_run_at: datetime | None) -> bool:
    """Check if current time matches cron expression.

    Format: minute hour day month weekday
    Supports: numbers, *, */N (every N)
    """
    if not cron_expr:
        return False

    now = datetime.now()

    # Don't trigger if we already ran this minute
    if last_run_at and (now - last_run_at).total_seconds() < 60:
        return False

    parts = cron_expr.split()
    if len(parts) != 5:
        return False

    minute, hour, day, month, weekday = parts

    return (
        _matches_field(minute, now.minute)
        and _matches_field(hour, now.hour)
        and _matches_field(day, now.day)
        and _matches_field(month, now.month)
        and _matches_field(weekday, now.weekday())  # 0=Monday
    )


def _matches_field(pattern: str, value: int) -> bool:
    """Check if a cron field matches a value."""
    if pattern == "*":
        return True

    if pattern.startswith("*/"):
        try:
            step = int(pattern[2:])
            return value % step == 0
        except ValueError:
            return False

    try:
        return int(pattern) == value
    except ValueError:
        return False
