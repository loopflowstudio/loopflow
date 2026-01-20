"""Tests for loopflow.lf.roadmap module."""

from datetime import datetime
from pathlib import Path

from loopflow.lf.roadmap import (
    Roadmap,
    RoadmapItem,
    approve_item,
    complete_item,
    create_item,
    format_roadmap_list,
    list_areas,
    load_roadmap,
    start_item,
)


def test_load_roadmap_empty_when_no_directory(tmp_path):
    """load_roadmap returns empty roadmap when .docs/roadmap/ doesn't exist."""
    repo = tmp_path / "repo"
    repo.mkdir()

    roadmap = load_roadmap(repo)

    assert roadmap.items == []


def test_load_roadmap_finds_items(tmp_path):
    """load_roadmap finds markdown files in area directories."""
    repo = tmp_path / "repo"
    roadmap_dir = repo / ".docs" / "roadmap" / "api"
    roadmap_dir.mkdir(parents=True)
    (roadmap_dir / "rate-limiting.md").write_text("""---
status: approved
area: api
---

# Add rate limiting

Implement rate limiting for the API.
""")

    roadmap = load_roadmap(repo)

    assert len(roadmap.items) == 1
    assert roadmap.items[0].title == "Add rate limiting"
    assert roadmap.items[0].status == "approved"
    assert roadmap.items[0].area == "api"


def test_load_roadmap_skips_done_directory(tmp_path):
    """load_roadmap ignores the _done/ archive directory."""
    repo = tmp_path / "repo"
    roadmap_dir = repo / ".docs" / "roadmap"
    api_dir = roadmap_dir / "api"
    done_dir = roadmap_dir / "_done"
    api_dir.mkdir(parents=True)
    done_dir.mkdir(parents=True)
    (api_dir / "active.md").write_text("---\nstatus: proposed\n---\n# Active")
    (done_dir / "old.md").write_text("---\nstatus: done\n---\n# Old")

    roadmap = load_roadmap(repo)

    assert len(roadmap.items) == 1
    assert roadmap.items[0].title == "Active"


def test_load_roadmap_extracts_title_from_h1(tmp_path):
    """Title is extracted from first H1 in body."""
    repo = tmp_path / "repo"
    roadmap_dir = repo / ".docs" / "roadmap" / "core"
    roadmap_dir.mkdir(parents=True)
    (roadmap_dir / "feature.md").write_text("""---
status: proposed
---

# My Feature Title

Some description.
""")

    roadmap = load_roadmap(repo)

    assert roadmap.items[0].title == "My Feature Title"


def test_load_roadmap_uses_filename_as_fallback_title(tmp_path):
    """Title falls back to slugified filename when no H1."""
    repo = tmp_path / "repo"
    roadmap_dir = repo / ".docs" / "roadmap" / "ui"
    roadmap_dir.mkdir(parents=True)
    (roadmap_dir / "dark-mode.md").write_text("---\nstatus: proposed\n---\nNo heading here")

    roadmap = load_roadmap(repo)

    assert roadmap.items[0].title == "Dark Mode"


def test_roadmap_for_area(tmp_path):
    """Roadmap.for_area filters by area."""
    repo = tmp_path / "repo"
    roadmap_dir = repo / ".docs" / "roadmap"
    (roadmap_dir / "api").mkdir(parents=True)
    (roadmap_dir / "ui").mkdir(parents=True)
    (roadmap_dir / "api" / "one.md").write_text("---\nstatus: proposed\narea: api\n---\n# One")
    (roadmap_dir / "ui" / "two.md").write_text("---\nstatus: proposed\narea: ui\n---\n# Two")

    roadmap = load_roadmap(repo)

    api_items = roadmap.for_area("api")
    assert len(api_items) == 1
    assert api_items[0].title == "One"


def test_roadmap_by_status(tmp_path):
    """Roadmap.by_status filters by status."""
    repo = tmp_path / "repo"
    roadmap_dir = repo / ".docs" / "roadmap" / "api"
    roadmap_dir.mkdir(parents=True)
    (roadmap_dir / "approved.md").write_text("---\nstatus: approved\n---\n# Approved")
    (roadmap_dir / "proposed.md").write_text("---\nstatus: proposed\n---\n# Proposed")

    roadmap = load_roadmap(repo)

    approved = roadmap.by_status("approved")
    assert len(approved) == 1
    assert approved[0].title == "Approved"


def test_roadmap_depth(tmp_path):
    """Roadmap.depth counts approved items."""
    repo = tmp_path / "repo"
    roadmap_dir = repo / ".docs" / "roadmap" / "api"
    roadmap_dir.mkdir(parents=True)
    (roadmap_dir / "a.md").write_text("---\nstatus: approved\n---\n# A")
    (roadmap_dir / "b.md").write_text("---\nstatus: approved\n---\n# B")
    (roadmap_dir / "c.md").write_text("---\nstatus: proposed\n---\n# C")

    roadmap = load_roadmap(repo)

    assert roadmap.depth() == 2
    assert roadmap.depth("api") == 2


def test_list_areas(tmp_path):
    """list_areas returns area directory names."""
    repo = tmp_path / "repo"
    roadmap_dir = repo / ".docs" / "roadmap"
    (roadmap_dir / "api").mkdir(parents=True)
    (roadmap_dir / "ui").mkdir(parents=True)
    (roadmap_dir / "_done").mkdir(parents=True)

    areas = list_areas(repo)

    assert areas == ["api", "ui"]


def test_create_item(tmp_path):
    """create_item creates a new roadmap item file."""
    repo = tmp_path / "repo"
    repo.mkdir()

    item = create_item(repo, "api", "rate limiting", "Add Rate Limiting", "Implement rate limiting.")

    assert item.status == "proposed"
    assert item.area == "api"
    assert item.title == "Add Rate Limiting"
    assert item.path.exists()
    content = item.path.read_text()
    assert "status: proposed" in content
    assert "# Add Rate Limiting" in content


def test_create_item_slugifies_name(tmp_path):
    """create_item converts name to slug for filename."""
    repo = tmp_path / "repo"
    repo.mkdir()

    item = create_item(repo, "api", "Add Rate Limiting!", "Title", "Content")

    assert item.path.name == "add-rate-limiting.md"


def test_approve_item(tmp_path):
    """approve_item marks item as approved."""
    repo = tmp_path / "repo"
    repo.mkdir()
    item = create_item(repo, "api", "test", "Test", "Content")

    approve_item(item)

    assert item.status == "approved"
    assert item.approved_at is not None
    content = item.path.read_text()
    assert "status: approved" in content


def test_start_item(tmp_path):
    """start_item marks in-progress and copies to .design/."""
    repo = tmp_path / "repo"
    repo.mkdir()
    item = create_item(repo, "api", "test", "Test Feature", "Content")

    start_item(item, "my-branch", repo)

    assert item.status == "in-progress"
    design_path = repo / ".design" / "my-branch.md"
    assert design_path.exists()
    assert "Test Feature" in design_path.read_text()


def test_complete_item_moves_to_done(tmp_path):
    """complete_item moves file to _done/ directory."""
    repo = tmp_path / "repo"
    repo.mkdir()
    item = create_item(repo, "api", "test", "Test", "Content")
    original_path = item.path

    complete_item(item, repo)

    assert not original_path.exists()
    done_path = repo / ".docs" / "roadmap" / "_done" / "test.md"
    assert done_path.exists()


def test_format_roadmap_list_empty():
    """format_roadmap_list handles empty roadmap."""
    roadmap = Roadmap(items=[])

    output = format_roadmap_list(roadmap)

    assert "No roadmap items found" in output


def test_format_roadmap_list_groups_by_area(tmp_path):
    """format_roadmap_list groups items by area and status."""
    repo = tmp_path / "repo"
    roadmap_dir = repo / ".docs" / "roadmap"
    (roadmap_dir / "api").mkdir(parents=True)
    (roadmap_dir / "ui").mkdir(parents=True)
    (roadmap_dir / "api" / "one.md").write_text("---\nstatus: approved\n---\n# One")
    (roadmap_dir / "ui" / "two.md").write_text("---\nstatus: proposed\n---\n# Two")

    roadmap = load_roadmap(repo)
    output = format_roadmap_list(roadmap)

    assert "api/" in output
    assert "ui/" in output
    assert "[approved]" in output
    assert "[proposed]" in output


def test_load_roadmap_handles_missing_frontmatter(tmp_path):
    """Items without frontmatter default to proposed status."""
    repo = tmp_path / "repo"
    roadmap_dir = repo / ".docs" / "roadmap" / "api"
    roadmap_dir.mkdir(parents=True)
    (roadmap_dir / "no-frontmatter.md").write_text("# Just a heading\n\nNo frontmatter here.")

    roadmap = load_roadmap(repo)

    assert len(roadmap.items) == 1
    assert roadmap.items[0].status == "proposed"


def test_load_roadmap_validates_status(tmp_path):
    """Invalid status values default to proposed."""
    repo = tmp_path / "repo"
    roadmap_dir = repo / ".docs" / "roadmap" / "api"
    roadmap_dir.mkdir(parents=True)
    (roadmap_dir / "bad-status.md").write_text("---\nstatus: invalid\n---\n# Test")

    roadmap = load_roadmap(repo)

    assert roadmap.items[0].status == "proposed"
