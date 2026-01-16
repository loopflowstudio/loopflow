"""Work item data structures."""

from dataclasses import dataclass, field
from typing import Literal


Status = Literal["proposed", "approved", "active", "done"]
ClaimedBy = Literal["human", "agent"]
Confidence = Literal["high", "medium", "low"]


@dataclass
class WorkItem:
    id: str
    title: str
    description: str
    status: Status = "proposed"
    claimed_by: ClaimedBy | None = None
    blocked_on: str | None = None
    confidence: Confidence = "medium"
    worktree: str | None = None
    notes: str = ""


def get_next_work(items: list[WorkItem]) -> WorkItem | None:
    """Pick work biased toward high-confidence, non-blocked items."""
    candidates = [
        c for c in items
        if c.status in ("approved", "active")
        and c.claimed_by != "human"
    ]

    if not candidates:
        return None

    # Sort: non-blocked first, then by confidence
    confidence_order = {"high": 0, "medium": 1, "low": 2}
    candidates.sort(key=lambda c: (
        c.blocked_on is not None,
        confidence_order.get(c.confidence, 1),
    ))

    return candidates[0]
