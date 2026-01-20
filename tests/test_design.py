"""Tests for loopflow.design helpers."""

from pathlib import Path

from loopflow.lf.design import (
    clear_design_artifacts,
    gather_design_docs,
    gather_internal_docs,
    has_design_artifacts,
    load_goal,
)


def test_gather_design_docs_reads_markdown(tmp_path):
    """gather_design_docs returns .design markdown files."""
    repo_root = tmp_path / "repo"
    repo_root.mkdir()
    design_dir = repo_root / ".design"
    design_dir.mkdir()
    (design_dir / "intent.md").write_text("# Intent\n")
    (design_dir / "notes.txt").write_text("ignore")

    docs = gather_design_docs(repo_root)

    assert len(docs) == 1
    path, content = docs[0]
    assert path == design_dir / "intent.md"
    assert "# Intent" in content


def test_clear_design_artifacts_keeps_folder(tmp_path):
    """clear_design_artifacts removes contents but keeps .design."""
    repo_root = tmp_path / "repo"
    repo_root.mkdir()
    design_dir = repo_root / ".design"
    design_dir.mkdir()
    (design_dir / "plan.md").write_text("Plan")
    (design_dir / "nested").mkdir()
    (design_dir / "nested" / "more.md").write_text("More")

    assert has_design_artifacts(repo_root) is True
    removed = clear_design_artifacts(repo_root)

    assert removed is True
    assert design_dir.exists() is True
    assert list(design_dir.iterdir()) == []
    assert has_design_artifacts(repo_root) is False


# =============================================================================
# Internal docs (.docs/) tests
# =============================================================================


def test_gather_internal_docs_reads_markdown(tmp_path):
    """gather_internal_docs returns .docs markdown files."""
    repo_root = tmp_path / "repo"
    repo_root.mkdir()
    docs_dir = repo_root / ".docs"
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
    docs_dir = repo_root / ".docs"
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
    """gather_internal_docs returns empty list when .docs/ doesn't exist."""
    repo_root = tmp_path / "repo"
    repo_root.mkdir()

    docs = gather_internal_docs(repo_root)

    assert docs == []


def test_gather_internal_docs_sorted(tmp_path):
    """gather_internal_docs returns files in sorted order."""
    repo_root = tmp_path / "repo"
    repo_root.mkdir()
    docs_dir = repo_root / ".docs"
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
    """load_goal finds goal by name in .lf/goals/."""
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

    from pathlib import Path
    content = load_goal(Path(".lf/goals/refactor.md"), repo_root)

    assert content is not None
    assert "# Refactor Goal" in content
