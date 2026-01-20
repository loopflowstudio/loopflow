"""File-based work backend using .todo/ directory."""

import re
from pathlib import Path
from typing import Any

from loopflow.lfd.work.models import WorkItem

_FRONTMATTER_PATTERN = re.compile(r"^---\s*\n(.*?)\n---\s*\n?", re.DOTALL)


def _slugify(text: str) -> str:
    """Convert text to a slug suitable for filenames."""
    slug = text.lower()
    slug = re.sub(r"[^a-z0-9]+", "-", slug)
    slug = slug.strip("-")
    return slug[:50] if slug else "untitled"


def _parse_frontmatter(text: str) -> tuple[dict[str, Any], str]:
    """Parse YAML frontmatter from text. Returns (metadata, content)."""
    match = _FRONTMATTER_PATTERN.match(text)
    if not match:
        return {}, text

    frontmatter = match.group(1)
    content = text[match.end() :]

    metadata: dict[str, Any] = {}
    for line in frontmatter.split("\n"):
        if ":" not in line:
            continue
        key, _, value = line.partition(":")
        key = key.strip()
        value = value.strip()

        if value.lower() == "null" or value == "":
            metadata[key] = None
        elif value.lower() in ("true", "yes"):
            metadata[key] = True
        elif value.lower() in ("false", "no"):
            metadata[key] = False
        else:
            metadata[key] = value

    return metadata, content


def _format_frontmatter(item: WorkItem) -> str:
    """Format a WorkItem as markdown with YAML frontmatter."""

    def fmt(val: Any) -> str:
        if val is None:
            return "null"
        if isinstance(val, bool):
            return "true" if val else "false"
        return str(val)

    lines = [
        "---",
        f"status: {item.status}",
        f"claimed_by: {fmt(item.claimed_by)}",
        f"blocked_on: {fmt(item.blocked_on)}",
        f"worktree: {fmt(item.worktree)}",
        "---",
        f"# {item.title}",
        "",
        item.description,
    ]

    if item.notes:
        lines.extend(["", "## Notes", "", item.notes])

    return "\n".join(lines)


def _parse_work_item(file_path: Path) -> WorkItem | None:
    """Parse a work item from a markdown file."""
    if not file_path.exists():
        return None

    text = file_path.read_text()
    metadata, content = _parse_frontmatter(text)

    # Extract title from first # heading
    title = ""
    description_lines = []
    notes = ""
    in_notes = False

    for line in content.split("\n"):
        if line.startswith("# ") and not title:
            title = line[2:].strip()
        elif line.strip() == "## Notes":
            in_notes = True
        elif in_notes:
            notes += line + "\n"
        elif title:
            description_lines.append(line)

    description = "\n".join(description_lines).strip()
    notes = notes.strip()

    return WorkItem(
        id=file_path.stem,
        title=title or file_path.stem,
        description=description,
        status=metadata.get("status", "proposed"),
        claimed_by=metadata.get("claimed_by"),
        blocked_on=metadata.get("blocked_on"),
        worktree=metadata.get("worktree"),
        notes=notes,
    )


class FileBackend:
    """File-based work backend using .todo/ directory."""

    def __init__(self, repo_root: Path):
        self.todo_dir = repo_root / ".todo"

    def _ensure_dir(self) -> None:
        self.todo_dir.mkdir(exist_ok=True)

    def list_items(self, status: str | None = None) -> list[WorkItem]:
        if not self.todo_dir.exists():
            return []

        items = []
        for path in self.todo_dir.glob("*.md"):
            item = _parse_work_item(path)
            if item:
                if status is None or item.status == status:
                    items.append(item)
        return items

    def get_item(self, item_id: str) -> WorkItem | None:
        path = self.todo_dir / f"{item_id}.md"
        return _parse_work_item(path)

    def create_item(self, item: WorkItem) -> WorkItem:
        self._ensure_dir()

        # Generate ID if not provided or already exists
        if not item.id or (self.todo_dir / f"{item.id}.md").exists():
            item.id = _slugify(item.title)
            # Ensure unique
            base_id = item.id
            counter = 1
            while (self.todo_dir / f"{item.id}.md").exists():
                item.id = f"{base_id}-{counter}"
                counter += 1

        path = self.todo_dir / f"{item.id}.md"
        path.write_text(_format_frontmatter(item))
        return item

    def update_item(self, item_id: str, **fields) -> WorkItem | None:
        item = self.get_item(item_id)
        if not item:
            return None

        # Update fields
        for key, value in fields.items():
            if hasattr(item, key):
                setattr(item, key, value)

        path = self.todo_dir / f"{item_id}.md"
        path.write_text(_format_frontmatter(item))
        return item

    def delete_item(self, item_id: str) -> bool:
        path = self.todo_dir / f"{item_id}.md"
        if path.exists():
            path.unlink()
            return True
        return False
