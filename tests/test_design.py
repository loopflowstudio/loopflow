"""Tests for loopflow.design helpers."""

from pathlib import Path

from loopflow.lf.design import clear_design_artifacts, gather_design_docs, has_design_artifacts


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
