"""Tests for loopflow.design helpers."""

from pathlib import Path

from loopflow.lf.design import (
    _area_parent_paths,
    clear_design_artifacts,
    gather_area_docs,
    gather_design_docs,
    gather_internal_docs,
    has_design_artifacts,
    load_goal,
)


def test_gather_design_docs_reads_markdown(tmp_path):
    """gather_design_docs returns scratch/ markdown files."""
    repo_root = tmp_path / "repo"
    repo_root.mkdir()
    scratch_dir = repo_root / "scratch"
    scratch_dir.mkdir()
    (scratch_dir / "intent.md").write_text("# Intent\n")
    (scratch_dir / "notes.txt").write_text("ignore")

    docs = gather_design_docs(repo_root)

    assert len(docs) == 1
    path, content = docs[0]
    assert path == scratch_dir / "intent.md"
    assert "# Intent" in content


def test_clear_design_artifacts_keeps_folder(tmp_path):
    """clear_design_artifacts removes contents but keeps scratch/."""
    repo_root = tmp_path / "repo"
    repo_root.mkdir()
    scratch_dir = repo_root / "scratch"
    scratch_dir.mkdir()
    (scratch_dir / "plan.md").write_text("Plan")
    (scratch_dir / "nested").mkdir()
    (scratch_dir / "nested" / "more.md").write_text("More")

    assert has_design_artifacts(repo_root) is True
    removed = clear_design_artifacts(repo_root)

    assert removed is True
    assert scratch_dir.exists() is True
    assert list(scratch_dir.iterdir()) == []
    assert has_design_artifacts(repo_root) is False


# =============================================================================
# Internal docs (roadmap/) tests
# =============================================================================


def test_gather_internal_docs_reads_markdown(tmp_path):
    """gather_internal_docs returns roadmap/ markdown files."""
    repo_root = tmp_path / "repo"
    repo_root.mkdir()
    docs_dir = repo_root / "roadmap"
    docs_dir.mkdir()
    (docs_dir / "architecture.md").write_text("# Architecture\n")
    (docs_dir / "notes.txt").write_text("ignore")

    docs = gather_internal_docs(repo_root)

    assert len(docs) == 1
    path, content = docs[0]
    assert path == docs_dir / "architecture.md"
    assert "# Architecture" in content


def test_gather_internal_docs_recursive(tmp_path):
    """gather_internal_docs finds nested markdown files."""
    repo_root = tmp_path / "repo"
    repo_root.mkdir()
    docs_dir = repo_root / "roadmap"
    docs_dir.mkdir()
    (docs_dir / "overview.md").write_text("# Overview\n")
    decisions = docs_dir / "decisions"
    decisions.mkdir()
    (decisions / "adr-001.md").write_text("# ADR 001\n")
    (decisions / "adr-002.md").write_text("# ADR 002\n")

    docs = gather_internal_docs(repo_root)

    assert len(docs) == 3
    paths = {str(p) for p, _ in docs}
    assert any("overview.md" in p for p in paths)
    assert any("adr-001.md" in p for p in paths)
    assert any("adr-002.md" in p for p in paths)


def test_gather_internal_docs_empty_when_missing(tmp_path):
    """gather_internal_docs returns empty list when roadmap/ doesn't exist."""
    repo_root = tmp_path / "repo"
    repo_root.mkdir()

    docs = gather_internal_docs(repo_root)

    assert docs == []


def test_gather_internal_docs_sorted(tmp_path):
    """gather_internal_docs returns files in sorted order."""
    repo_root = tmp_path / "repo"
    repo_root.mkdir()
    docs_dir = repo_root / "roadmap"
    docs_dir.mkdir()
    (docs_dir / "zebra.md").write_text("Z")
    (docs_dir / "alpha.md").write_text("A")
    (docs_dir / "beta.md").write_text("B")

    docs = gather_internal_docs(repo_root)

    names = [p.name for p, _ in docs]
    assert names == ["alpha.md", "beta.md", "zebra.md"]


# =============================================================================
# Goal loading tests
# =============================================================================


def test_load_goal_by_name(tmp_path):
    """load_goal finds goal by name in lf/goals/."""
    repo_root = tmp_path / "repo"
    repo_root.mkdir()
    goals_dir = repo_root / ".lf" / "goals"
    goals_dir.mkdir(parents=True)
    (goals_dir / "test-coverage.md").write_text("# Test Coverage\n\nImprove coverage.\n")

    content = load_goal("test-coverage", repo_root)

    assert content is not None
    assert "# Test Coverage" in content
    assert "Improve coverage." in content


def test_load_goal_by_path(tmp_path):
    """load_goal finds goal by explicit path."""
    repo_root = tmp_path / "repo"
    repo_root.mkdir()
    goals_dir = repo_root / ".lf" / "goals"
    goals_dir.mkdir(parents=True)
    (goals_dir / "security.md").write_text("# Security Goal\n")

    content = load_goal(".lf/goals/security.md", repo_root)

    assert content is not None
    assert "# Security Goal" in content


def test_load_goal_returns_none_when_missing(tmp_path):
    """load_goal returns None for non-existent goal."""
    repo_root = tmp_path / "repo"
    repo_root.mkdir()

    content = load_goal("nonexistent", repo_root)

    assert content is None


def test_load_goal_with_path_object(tmp_path):
    """load_goal accepts Path objects."""
    repo_root = tmp_path / "repo"
    repo_root.mkdir()
    goals_dir = repo_root / ".lf" / "goals"
    goals_dir.mkdir(parents=True)
    (goals_dir / "refactor.md").write_text("# Refactor Goal\n")

    content = load_goal(Path(".lf/goals/refactor.md"), repo_root)

    assert content is not None
    assert "# Refactor Goal" in content


# =============================================================================
# Area docs tests
# =============================================================================


def test_area_parent_paths_single():
    """_area_parent_paths returns single path for simple area."""
    assert _area_parent_paths("lf") == ["lf"]


def test_area_parent_paths_nested():
    """_area_parent_paths returns all parent paths."""
    assert _area_parent_paths("a/b/c") == ["a", "a/b", "a/b/c"]


def test_area_parent_paths_handles_slashes():
    """_area_parent_paths handles leading/trailing slashes."""
    assert _area_parent_paths("/a/b/") == ["a", "a/b"]


def test_gather_area_docs_includes_direct_md(tmp_path):
    """gather_area_docs includes .md files directly in area folders."""
    repo_root = tmp_path / "repo"
    repo_root.mkdir()
    (repo_root / "lf").mkdir()
    (repo_root / "lf" / "README.md").write_text("# LF docs")
    (repo_root / "lf" / "cli").mkdir()
    (repo_root / "lf" / "cli" / "GUIDE.md").write_text("# CLI guide")

    docs = gather_area_docs(repo_root, "lf/cli")

    names = [p.name for p, _ in docs]
    assert "README.md" in names  # from lf/
    assert "GUIDE.md" in names  # from lf/cli/


def test_gather_area_docs_includes_roadmap(tmp_path):
    """gather_area_docs includes roadmap/ under each area."""
    repo_root = tmp_path / "repo"
    repo_root.mkdir()
    (repo_root / "lf").mkdir()
    (repo_root / "lf" / "roadmap").mkdir()
    (repo_root / "lf" / "roadmap" / "feature.md").write_text("# Feature plan")
    (repo_root / "lf" / "cli").mkdir()
    (repo_root / "lf" / "cli" / "roadmap").mkdir()
    (repo_root / "lf" / "cli" / "roadmap" / "refactor.md").write_text("# CLI refactor")

    docs = gather_area_docs(repo_root, "lf/cli")

    names = [p.name for p, _ in docs]
    assert "feature.md" in names  # from lf/roadmap/
    assert "refactor.md" in names  # from lf/cli/roadmap/


def test_gather_area_docs_nested_roadmap(tmp_path):
    """gather_area_docs includes nested docs within roadmap/."""
    repo_root = tmp_path / "repo"
    repo_root.mkdir()
    (repo_root / "lf").mkdir()
    (repo_root / "lf" / "roadmap").mkdir()
    (repo_root / "lf" / "roadmap" / "decisions").mkdir()
    (repo_root / "lf" / "roadmap" / "decisions" / "adr-001.md").write_text("# ADR")

    docs = gather_area_docs(repo_root, "lf")

    names = [p.name for p, _ in docs]
    assert "adr-001.md" in names


def test_gather_area_docs_empty_when_no_area_exists(tmp_path):
    """gather_area_docs returns empty list when area doesn't exist."""
    repo_root = tmp_path / "repo"
    repo_root.mkdir()

    docs = gather_area_docs(repo_root, "nonexistent/area")

    assert docs == []


def test_gather_area_docs_no_duplicates(tmp_path):
    """gather_area_docs doesn't include the same file twice."""
    repo_root = tmp_path / "repo"
    repo_root.mkdir()
    (repo_root / "lf").mkdir()
    (repo_root / "lf" / "README.md").write_text("# LF")

    docs = gather_area_docs(repo_root, "lf")

    # Should only appear once
    readme_count = sum(1 for p, _ in docs if p.name == "README.md")
    assert readme_count == 1
