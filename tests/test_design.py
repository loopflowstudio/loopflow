"""Tests for loopflow.design helpers."""

from pathlib import Path

from loopflow.lf.design import (
    clear_design_artifacts,
    gather_design_docs,
    gather_internal_docs,
    has_design_artifacts,
    load_voice,
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
# Voice loading tests
# =============================================================================


def test_load_voice_by_name(tmp_path):
    """load_voice finds voice by name in lf/voices/."""
    repo_root = tmp_path / "repo"
    repo_root.mkdir()
    voices_dir = repo_root / ".lf" / "voices"
    voices_dir.mkdir(parents=True)
    (voices_dir / "test-coverage.md").write_text("# Test Coverage\n\nImprove coverage.\n")

    content = load_voice("test-coverage", repo_root)

    assert content is not None
    assert "# Test Coverage" in content
    assert "Improve coverage." in content


def test_load_voice_by_path(tmp_path):
    """load_voice finds voice by explicit path."""
    repo_root = tmp_path / "repo"
    repo_root.mkdir()
    voices_dir = repo_root / ".lf" / "voices"
    voices_dir.mkdir(parents=True)
    (voices_dir / "security.md").write_text("# Security Voice\n")

    content = load_voice(".lf/voices/security.md", repo_root)

    assert content is not None
    assert "# Security Voice" in content


def test_load_voice_returns_none_when_missing(tmp_path):
    """load_voice returns None for non-existent voice."""
    repo_root = tmp_path / "repo"
    repo_root.mkdir()

    content = load_voice("nonexistent", repo_root)

    assert content is None


def test_load_voice_with_path_object(tmp_path):
    """load_voice accepts Path objects."""
    repo_root = tmp_path / "repo"
    repo_root.mkdir()
    voices_dir = repo_root / ".lf" / "voices"
    voices_dir.mkdir(parents=True)
    (voices_dir / "refactor.md").write_text("# Refactor Voice\n")

    content = load_voice(Path(".lf/voices/refactor.md"), repo_root)

    assert content is not None
    assert "# Refactor Voice" in content
