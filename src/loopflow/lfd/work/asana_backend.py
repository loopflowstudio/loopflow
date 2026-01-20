"""Asana-based work backend."""

import logging
import os
from typing import Any

import asana

from loopflow.lfd.work.models import Status, WorkItem

logger = logging.getLogger(__name__)


# Section name to status mapping
_SECTION_TO_STATUS: dict[str, Status] = {
    "proposed": "proposed",
    "approved": "approved",
    "active": "active",
    "done": "done",
}

_STATUS_TO_SECTION: dict[Status, str] = {v: k for k, v in _SECTION_TO_STATUS.items()}


class AsanaBackend:
    """Asana-based work backend."""

    def __init__(self, project_id: str, access_token: str | None = None):
        self.project_id = project_id
        self.access_token = access_token or os.environ.get("ASANA_ACCESS_TOKEN")
        self._client: Any = None
        self._sections: dict[str, str] = {}  # section_name -> section_gid

    def _get_client(self):
        if self._client is None:
            if not self.access_token:
                raise RuntimeError("ASANA_ACCESS_TOKEN not set")

            self._client = asana.Client.access_token(self.access_token)
        return self._client

    def _load_sections(self) -> None:
        """Load section GIDs for status mapping."""
        if self._sections:
            return

        client = self._get_client()
        sections = client.sections.get_sections_for_project(self.project_id)
        for section in sections:
            name = section["name"].lower()
            if name in _SECTION_TO_STATUS:
                self._sections[name] = section["gid"]

    def _get_section_gid(self, status: Status) -> str | None:
        self._load_sections()
        section_name = _STATUS_TO_SECTION.get(status, "").lower()
        return self._sections.get(section_name)

    def _task_to_work_item(self, task: dict) -> WorkItem:
        """Convert Asana task to WorkItem."""
        # Determine status from section
        status: Status = "proposed"
        memberships = task.get("memberships", [])
        for m in memberships:
            section = m.get("section", {})
            section_name = section.get("name", "").lower()
            if section_name in _SECTION_TO_STATUS:
                status = _SECTION_TO_STATUS[section_name]
                break

        # Check for "human" tag -> claimed_by
        claimed_by = None
        for tag in task.get("tags", []):
            if tag.get("name", "").lower() == "human":
                claimed_by = "human"
                break

        # Get custom fields
        blocked_on = None
        worktree = None
        for cf in task.get("custom_fields", []):
            name = cf.get("name", "").lower()
            if name == "blocked_on":
                blocked_on = cf.get("text_value")
            elif name == "worktree":
                worktree = cf.get("text_value")

        # Get subtasks and append to description
        description = task.get("notes", "")
        subtasks = task.get("subtasks", [])
        if subtasks:
            subtask_lines = [f"- {st.get('name', '')}" for st in subtasks]
            description += "\n\n## Subtasks\n" + "\n".join(subtask_lines)

        # Get notes from comments (simplified - just get recent ones)
        notes = ""

        return WorkItem(
            id=task["gid"],
            title=task.get("name", ""),
            description=description,
            status=status,
            claimed_by=claimed_by,
            blocked_on=blocked_on,
            worktree=worktree,
            notes=notes,
        )

    def list_items(self, status: str | None = None) -> list[WorkItem]:
        client = self._get_client()

        # Fetch tasks with needed fields
        tasks = client.tasks.get_tasks_for_project(
            self.project_id,
            opt_fields=[
                "name",
                "notes",
                "completed",
                "memberships.section.name",
                "tags.name",
                "custom_fields",
                "subtasks.name",
            ],
        )

        items = []
        for task in tasks:
            if task.get("completed"):
                continue
            item = self._task_to_work_item(task)
            if status is None or item.status == status:
                items.append(item)

        return items

    def get_item(self, item_id: str) -> WorkItem | None:
        client = self._get_client()
        try:
            task = client.tasks.get_task(
                item_id,
                opt_fields=[
                    "name",
                    "notes",
                    "completed",
                    "memberships.section.name",
                    "tags.name",
                    "custom_fields",
                    "subtasks.name",
                ],
            )
            if task.get("completed"):
                return None
            return self._task_to_work_item(task)
        except Exception as e:
            logger.debug("Failed to get Asana task %s: %s", item_id, e)
            return None

    def create_item(self, item: WorkItem) -> WorkItem:
        client = self._get_client()

        task_data: dict[str, Any] = {
            "name": item.title,
            "notes": item.description,
            "projects": [self.project_id],
        }

        task = client.tasks.create_task(task_data)
        item.id = task["gid"]

        # Move to correct section
        section_gid = self._get_section_gid(item.status)
        if section_gid:
            client.sections.add_task_for_section(section_gid, {"task": item.id})

        return item

    def update_item(self, item_id: str, **fields) -> WorkItem | None:
        client = self._get_client()

        try:
            client.tasks.get_task(item_id)  # Verify task exists
        except Exception as e:
            logger.debug("Failed to get Asana task %s for update: %s", item_id, e)
            return None

        update_data: dict[str, Any] = {}

        if "title" in fields:
            update_data["name"] = fields["title"]
        if "description" in fields:
            update_data["notes"] = fields["description"]

        if update_data:
            client.tasks.update_task(item_id, update_data)

        # Handle status change by moving to section
        if "status" in fields:
            section_gid = self._get_section_gid(fields["status"])
            if section_gid:
                client.sections.add_task_for_section(section_gid, {"task": item_id})

        # Handle completion
        if fields.get("status") == "done":
            client.tasks.update_task(item_id, {"completed": True})

        return self.get_item(item_id)

    def delete_item(self, item_id: str) -> bool:
        client = self._get_client()
        try:
            client.tasks.delete_task(item_id)
            return True
        except Exception as e:
            logger.debug("Failed to delete Asana task %s: %s", item_id, e)
            return False
