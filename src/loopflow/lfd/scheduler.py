"""Scheduler for coordinating parallel loop execution."""

import threading
from dataclasses import dataclass
from pathlib import Path

import yaml

from loopflow.lfd.db import list_jobs
from loopflow.lfd.jobs import count_outstanding


@dataclass
class SchedulerConfig:
    """Configuration for the loop scheduler."""

    concurrency: int = 3  # Max parallel iterations
    global_pr_limit: int = 15  # Total outstanding across all loops


def load_scheduler_config() -> SchedulerConfig:
    """Load scheduler config from ~/.lf/lfd.yaml."""
    config_path = Path.home() / ".lf" / "lfd.yaml"
    if not config_path.exists():
        return SchedulerConfig()

    try:
        data = yaml.safe_load(config_path.read_text()) or {}
        return SchedulerConfig(
            concurrency=data.get("concurrency", 3),
            global_pr_limit=data.get("global_pr_limit", 15),
        )
    except Exception:
        return SchedulerConfig()


class Scheduler:
    """Coordinates multiple loops with shared resource limits.

    Thread-safe slot management for parallel iteration execution.
    """

    def __init__(self, config: SchedulerConfig | None = None):
        self.config = config or load_scheduler_config()
        self._running: set[str] = set()  # run IDs currently executing
        self._lock = threading.Lock()

    @property
    def concurrency(self) -> int:
        return self.config.concurrency

    @property
    def global_pr_limit(self) -> int:
        return self.config.global_pr_limit

    def slots_available(self) -> int:
        """Number of slots currently available."""
        with self._lock:
            return max(0, self.concurrency - len(self._running))

    def slots_used(self) -> int:
        """Number of slots currently in use."""
        with self._lock:
            return len(self._running)

    def total_outstanding(self) -> int:
        """Total outstanding commits across all loops."""
        total = 0
        for job in list_jobs():
            total += count_outstanding(job)
        return total

    def can_start(self) -> tuple[bool, str | None]:
        """Check if a new iteration can start.

        Returns (can_start, reason) where reason explains why if can't start.
        """
        with self._lock:
            if len(self._running) >= self.concurrency:
                return False, "concurrency"

        if self.total_outstanding() >= self.global_pr_limit:
            return False, "global_limit"

        return True, None

    def acquire(self, run_id: str) -> tuple[bool, str | None]:
        """Try to acquire a slot for an iteration.

        Returns (acquired, reason) where reason explains why if not acquired.
        """
        with self._lock:
            if len(self._running) >= self.concurrency:
                return False, "concurrency"

        if self.total_outstanding() >= self.global_pr_limit:
            return False, "global_limit"

        with self._lock:
            if len(self._running) >= self.concurrency:
                return False, "concurrency"
            self._running.add(run_id)
            return True, None

    def release(self, run_id: str) -> None:
        """Release a slot when iteration completes."""
        with self._lock:
            self._running.discard(run_id)

    def get_status(self) -> dict:
        """Get scheduler status for display."""
        with self._lock:
            slots_used = len(self._running)
            running_ids = list(self._running)

        return {
            "slots_used": slots_used,
            "slots_total": self.concurrency,
            "outstanding": self.total_outstanding(),
            "outstanding_limit": self.global_pr_limit,
            "running": running_ids,
        }


# Global scheduler instance for daemon
_scheduler: Scheduler | None = None


def get_scheduler() -> Scheduler:
    """Get or create the global scheduler instance."""
    global _scheduler
    if _scheduler is None:
        _scheduler = Scheduler()
    return _scheduler


def reset_scheduler() -> None:
    """Reset the global scheduler (for testing)."""
    global _scheduler
    _scheduler = None
