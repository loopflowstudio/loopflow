"""Roadmap loading and management for agent loops."""

import re
import shutil
from dataclasses import dataclass
from datetime import datetime
from pathlib import Path

from loopflow.lf.directions import _FRONTMATTER_PATTERN, _parse_frontmatter


@dataclass
class RoadmapItem:
    """A design spec for substantial work."""

    path: Path
    area: str
    status: str  # proposed | approved | in-progress | done
    title: str
    created_at: datetime
    approved_at: datetime | None = None


@dataclass
class Roadmap:
    """All roadmap items for a repo."""

    items: list[RoadmapItem]

    def for_area(self, area: str) -> list[RoadmapItem]:
        """Items matching an area."""
        return [item for item in self.items if item.area == area]

    def by_status(self, status: str) -> list[RoadmapItem]:
        """Items with a specific status."""
        return [item for item in self.items if item.status == status]

    def depth(self, area: str | None = None) -> int:
        """Count of approved items ready for building."""
        items = self.for_area(area) if area else self.items
        return len([i for i in items if i.status == "approved"])


def load_roadmap(repo: Path) -> Roadmap:
    """Load all roadmap items from roadmap/."""
    roadmap_dir = repo / "roadmap"
    if not roadmap_dir.exists():
        return Roadmap(items=[])

    items = []
    for area_dir in roadmap_dir.iterdir():
        if not area_dir.is_dir():
            continue
        # Skip _done archive
        if area_dir.name.startswith("_"):
            continue

        area = area_dir.name
        for item_path in area_dir.glob("*.md"):
            item = _parse_roadmap_item(item_path, area)
            if item:
                items.append(item)

    # Sort by created_at (oldest first for FIFO-ish behavior, though agent decides)
    items.sort(key=lambda x: x.created_at)
    return Roadmap(items=items)


def _parse_roadmap_item(path: Path, area: str) -> RoadmapItem | None:
    """Parse a roadmap item file."""
    try:
        text = path.read_text()
    except Exception:
        return None

    frontmatter, body = _parse_frontmatter(text)

    # Extract title from first H1
    title = path.stem.replace("-", " ").replace("_", " ").title()
    for line in body.split("\n"):
        if line.startswith("# "):
            title = line[2:].strip()
            break

    status = frontmatter.get("status", "proposed")
    if status not in ("proposed", "approved", "in-progress", "done"):
        status = "proposed"

    # Use file mtime as created_at if not in frontmatter
    created_at = datetime.fromtimestamp(path.stat().st_mtime)
    if "created_at" in frontmatter:
        try:
            created_at = datetime.fromisoformat(frontmatter["created_at"])
        except (ValueError, TypeError):
            pass

    approved_at = None
    if "approved_at" in frontmatter:
        try:
            approved_at = datetime.fromisoformat(frontmatter["approved_at"])
        except (ValueError, TypeError):
            pass

    return RoadmapItem(
        path=path,
        area=frontmatter.get("area", area),
        status=status,
        title=title,
        created_at=created_at,
        approved_at=approved_at,
    )


def list_areas(repo: Path) -> list[str]:
    """List all waves in roadmap/."""
    roadmap_dir = repo / "roadmap"
    if not roadmap_dir.exists():
        return []

    areas = []
    for wave_dir in roadmap_dir.iterdir():
        if wave_dir.is_dir() and not wave_dir.name.startswith("_"):
            areas.append(wave_dir.name)
    return sorted(areas)


def start_item(item: RoadmapItem, branch: str, repo: Path) -> None:
    """Mark item in-progress, copy to scratch/<branch>.md."""
    # Update status in file
    _update_item_status(item, "in-progress")

    # Copy to scratch/
    design_dir = repo / "scratch"
    design_dir.mkdir(exist_ok=True)
    design_path = design_dir / f"{branch}.md"

    # Read original and write to design
    content = item.path.read_text()
    design_path.write_text(content)


def complete_item(item: RoadmapItem, repo: Path) -> None:
    """Move to _done/, clean up scratch/."""
    # Create _done directory if needed
    done_dir = item.path.parent.parent / "_done"
    done_dir.mkdir(exist_ok=True)

    # Move file to _done
    dest = done_dir / item.path.name
    shutil.move(str(item.path), str(dest))


def approve_item(item: RoadmapItem) -> None:
    """Mark item as approved."""
    _update_item_status(item, "approved", approved_at=datetime.now())


def _update_item_status(
    item: RoadmapItem, status: str, approved_at: datetime | None = None
) -> None:
    """Update the status in a roadmap item's frontmatter."""
    text = item.path.read_text()
    match = _FRONTMATTER_PATTERN.match(text)

    if not match:
        # No frontmatter, add it
        new_frontmatter = f"---\nstatus: {status}\narea: {item.area}\n"
        if approved_at:
            new_frontmatter += f"approved_at: {approved_at.isoformat()}\n"
        new_frontmatter += "---\n\n"
        item.path.write_text(new_frontmatter + text)
        return

    # Update existing frontmatter
    frontmatter_text = match.group(1)
    body = text[match.end() :]

    lines = []
    status_found = False
    approved_found = False
    for line in frontmatter_text.split("\n"):
        if line.startswith("status:"):
            lines.append(f"status: {status}")
            status_found = True
        elif line.startswith("approved_at:") and approved_at:
            lines.append(f"approved_at: {approved_at.isoformat()}")
            approved_found = True
        else:
            lines.append(line)

    if not status_found:
        lines.insert(0, f"status: {status}")
    if approved_at and not approved_found:
        lines.append(f"approved_at: {approved_at.isoformat()}")

    new_text = "---\n" + "\n".join(lines) + "\n---\n" + body
    item.path.write_text(new_text)
    item.status = status
    if approved_at:
        item.approved_at = approved_at


def create_item(repo: Path, area: str, name: str, title: str, content: str) -> RoadmapItem:
    """Create a new roadmap item."""
    roadmap_dir = repo / "roadmap" / area
    roadmap_dir.mkdir(parents=True, exist_ok=True)

    # Slugify name
    slug = name.lower().replace(" ", "-").replace("_", "-")
    slug = re.sub(r"[^a-z0-9-]", "", slug)
    path = roadmap_dir / f"{slug}.md"

    # Build file content
    now = datetime.now()
    file_content = f"""---
status: proposed
area: {area}
created_at: {now.isoformat()}
---

# {title}

{content}
"""
    path.write_text(file_content)

    return RoadmapItem(
        path=path,
        area=area,
        status="proposed",
        title=title,
        created_at=now,
        approved_at=None,
    )


def format_roadmap_list(roadmap: Roadmap) -> str:
    """Format roadmap for display."""
    if not roadmap.items:
        return "No items found in roadmap/"

    lines = []

    # Group by area
    areas: dict[str, list[RoadmapItem]] = {}
    for item in roadmap.items:
        areas.setdefault(item.area, []).append(item)

    for area in sorted(areas.keys()):
        items = areas[area]
        lines.append(f"\n{area}/")

        # Group by status within area
        by_status: dict[str, list[RoadmapItem]] = {}
        for item in items:
            by_status.setdefault(item.status, []).append(item)

        status_order = ["in-progress", "approved", "proposed"]
        for status in status_order:
            if status not in by_status:
                continue
            for item in by_status[status]:
                badge = f"[{status}]"
                rel_path = item.path.name
                lines.append(f"  {badge:14} {rel_path}: {item.title}")

    return "\n".join(lines)
