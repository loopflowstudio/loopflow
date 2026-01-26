"""Manager for coordinating parallel worker execution.

The manager controls how many workers can run simultaneously and
tracks global PR limits across all waves.
"""

import threading
from dataclasses import dataclass
from pathlib import Path

import yaml

from loopflow.lfd.wave import count_outstanding, list_waves


@dataclass
class ManagerConfig:
    """Configuration for the worker manager."""

    concurrency: int = 3  # Max parallel workers
    global_pr_limit: int = 15  # Total outstanding across all waves


def load_manager_config() -> ManagerConfig:
    """Load manager config from ~/.lf/lfd.yaml."""
    config_path = Path.home() / ".lf" / "lfd.yaml"
    if not config_path.exists():
        return ManagerConfig()

    try:
        data = yaml.safe_load(config_path.read_text()) or {}
        return ManagerConfig(
            concurrency=data.get("concurrency", 3),
            global_pr_limit=data.get("global_pr_limit", 15),
        )
    except Exception:
        return ManagerConfig()


class Manager:
    """Coordinates multiple workers with shared resource limits.

    Thread-safe slot management for parallel iteration execution.
    """

    def __init__(self, config: ManagerConfig | None = None):
        self.config = config or load_manager_config()
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
        """Total outstanding commits across all waves."""
        total = 0
        for wave in list_waves():
            total += count_outstanding(wave)
        return total

    def can_start(self) -> tuple[bool, str | None]:
        """Check if a new worker can start.

        Returns (can_start, reason) where reason explains why if can't start.
        """
        with self._lock:
            if len(self._running) >= self.concurrency:
                return False, "concurrency"

        if self.total_outstanding() >= self.global_pr_limit:
            return False, "global_limit"

        return True, None

    def acquire(self, run_id: str) -> tuple[bool, str | None]:
        """Try to acquire a slot for a worker.

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
        """Release a slot when worker completes."""
        with self._lock:
            self._running.discard(run_id)

    def get_status(self) -> dict:
        """Get manager status for display."""
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


# Global manager instance for daemon
_manager: Manager | None = None


def get_manager() -> Manager:
    """Get or create the global manager instance."""
    global _manager
    if _manager is None:
        _manager = Manager()
    return _manager


def reset_manager() -> None:
    """Reset the global manager (for testing)."""
    global _manager
    _manager = None
