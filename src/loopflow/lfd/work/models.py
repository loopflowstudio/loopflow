"""Work item data structures."""

from dataclasses import dataclass
from typing import Literal

Status = Literal["proposed", "approved", "active", "done"]
ClaimedBy = Literal["human", "agent"]


@dataclass
class WorkItem:
    id: str
    title: str
    description: str
    status: Status = "proposed"
    claimed_by: ClaimedBy | None = None
    blocked_on: str | None = None
    worktree: str | None = None
    notes: str = ""


def get_next_work(items: list[WorkItem]) -> WorkItem | None:
    """Pick non-blocked work items."""
    candidates = [
        c for c in items if c.status in ("approved", "active") and c.claimed_by != "human"
    ]

    if not candidates:
        return None

    # Sort: non-blocked first
    candidates.sort(key=lambda c: c.blocked_on is not None)

    return candidates[0]
