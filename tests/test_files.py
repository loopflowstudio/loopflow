"""Tests for loopflow.files module."""

from pathlib import Path

import pytest

from loopflow.files import (
    gather_files,
    format_files,
    _load_gitignore,
)


@pytest.fixture
def temp_repo(tmp_path):
    """Create a temporary repo with documentation and ignored files."""
    (tmp_path / ".git").mkdir()
    (tmp_path / ".gitignore").write_text("*.log\nbuild/\n")

    (tmp_path / "README.md").write_text("# Project\n")
    (tmp_path / "CONTRIBUTING.md").write_text("# Contributing\n")
    (tmp_path / "main.py").write_text("print('hello')\n")
    (tmp_path / "debug.log").write_text("logs\n")

    src = tmp_path / "src"
    src.mkdir()
    (src / "README.md").write_text("# Source\n")
    (src / "app.py").write_text("def main(): pass\n")

    build = tmp_path / "build"
    build.mkdir()
    (build / "output.txt").write_text("build output\n")

    _load_gitignore.cache_clear()
    return tmp_path


def test_gather_files_includes_file_with_parent_docs(temp_repo):
    """Requesting a file includes it plus parent .md documentation."""
    results = gather_files(["src/app.py"], temp_repo)
    paths = [p for p, _ in results]

    assert temp_repo / "README.md" in paths
    assert temp_repo / "CONTRIBUTING.md" in paths
    assert temp_repo / "src" / "README.md" in paths
    assert temp_repo / "src" / "app.py" in paths


def test_gather_files_orders_root_to_leaf(temp_repo):
    """Parent docs come before child docs, alphabetical within each directory."""
    results = gather_files(["src/app.py"], temp_repo)
    paths = [p for p, _ in results]

    # Root docs before src docs before file
    root_contrib = paths.index(temp_repo / "CONTRIBUTING.md")
    root_readme = paths.index(temp_repo / "README.md")
    src_readme = paths.index(temp_repo / "src" / "README.md")
    src_app = paths.index(temp_repo / "src" / "app.py")

    assert root_contrib < root_readme  # alphabetical in root
    assert root_readme < src_readme    # root before src
    assert src_readme < src_app        # docs before file


def test_gather_files_excludes_gitignored(temp_repo):
    """Gitignored files and directories are excluded."""
    results = gather_files(["debug.log", "build/output.txt", "main.py"], temp_repo)
    paths = [p for p, _ in results]

    assert temp_repo / "main.py" in paths
    assert temp_repo / "debug.log" not in paths
    assert temp_repo / "build" / "output.txt" not in paths


def test_gather_files_excludes_lf_directory(temp_repo):
    """The .lf directory is excluded (prompt config, not context)."""
    lf_dir = temp_repo / ".lf"
    lf_dir.mkdir()
    (lf_dir / "README.md").write_text("# LF Config\n")
    (lf_dir / "tasks").mkdir()
    (lf_dir / "tasks" / "review.lf").write_text("Review the code.\n")

    results = gather_files([".lf/README.md", ".lf/tasks/review.lf", "main.py"], temp_repo)
    paths = [p for p, _ in results]

    assert temp_repo / "main.py" in paths
    assert temp_repo / ".lf" / "README.md" not in paths
    assert temp_repo / ".lf" / "tasks" / "review.lf" not in paths


def test_gather_files_deduplicates_across_requests(temp_repo):
    """Multiple file requests don't duplicate shared parent docs."""
    results = gather_files(["main.py", "src/app.py"], temp_repo)
    paths = [p for p, _ in results]

    assert paths.count(temp_repo / "README.md") == 1


def test_format_files_uses_unique_delimiters(temp_repo):
    """Format uses <lf:tag> delimiters with preamble outside."""
    files = [(temp_repo / "src" / "app.py", "def main(): pass\n")]
    result = format_files(files, temp_repo)

    assert "Reference files" in result  # preamble outside
    assert "<lf:files>" in result
    assert '<lf:file path="src/app.py">' in result
    assert "def main(): pass" in result
    assert "</lf:file>" in result
    assert "</lf:files>" in result
