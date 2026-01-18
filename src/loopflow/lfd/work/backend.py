"""Work backend protocol and implementations."""

from typing import Protocol

from loopflow.lfd.work.models import WorkItem


class WorkBackend(Protocol):
    """Protocol for work item storage backends."""

    def list_items(self, status: str | None = None) -> list[WorkItem]:
        """List work items, optionally filtered by status."""
        ...

    def get_item(self, item_id: str) -> WorkItem | None:
        """Get a work item by ID."""
        ...

    def create_item(self, item: WorkItem) -> WorkItem:
        """Create a new work item."""
        ...

    def update_item(self, item_id: str, **fields) -> WorkItem | None:
        """Update a work item. Returns updated item or None if not found."""
        ...

    def delete_item(self, item_id: str) -> bool:
        """Delete a work item. Returns True if deleted."""
        ...
