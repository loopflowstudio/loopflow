"""Tests for loopflow.files module."""

from pathlib import Path

import pytest

from loopflow.files import (
    gather_files,
    format_files,
    is_binary,
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


# --- Path expression tests ---


def test_gather_files_directory_expands_to_all_files(temp_repo):
    """A directory path gathers all non-ignored files recursively."""
    results = gather_files(["src"], temp_repo)
    paths = [p for p, _ in results]

    assert temp_repo / "src" / "app.py" in paths
    assert temp_repo / "src" / "README.md" in paths


def test_gather_files_dot_expands_to_whole_repo(temp_repo):
    """'.' gathers all non-ignored files in the repo."""
    results = gather_files(["."], temp_repo)
    paths = [p for p, _ in results]

    assert temp_repo / "main.py" in paths
    assert temp_repo / "src" / "app.py" in paths
    assert temp_repo / "README.md" in paths
    # Ignored files still excluded
    assert temp_repo / "debug.log" not in paths
    assert temp_repo / "build" / "output.txt" not in paths


def test_gather_files_glob_pattern(temp_repo):
    """Glob patterns expand to matching files."""
    results = gather_files(["*.py"], temp_repo)
    paths = [p for p, _ in results]

    assert temp_repo / "main.py" in paths


def test_gather_files_recursive_glob_pattern(temp_repo):
    """Recursive glob patterns expand to matching files in subdirs."""
    results = gather_files(["**/*.py"], temp_repo)
    paths = [p for p, _ in results]

    assert temp_repo / "main.py" in paths
    assert temp_repo / "src" / "app.py" in paths


def test_gather_files_mixed_paths(temp_repo):
    """Mix of files, directories, and globs all work together."""
    results = gather_files(["main.py", "src", "*.md"], temp_repo)
    paths = [p for p, _ in results]

    assert temp_repo / "main.py" in paths
    assert temp_repo / "src" / "app.py" in paths
    assert temp_repo / "README.md" in paths
    assert temp_repo / "CONTRIBUTING.md" in paths


# --- Exclude pattern tests ---


def test_gather_files_exclude_pattern(temp_repo):
    """Exclude patterns filter out matching files."""
    results = gather_files(["main.py", "src/app.py"], temp_repo, exclude=["**/*.py"])
    paths = [p for p, _ in results]

    # Python files excluded (using **/*.py for all levels)
    assert temp_repo / "main.py" not in paths
    assert temp_repo / "src" / "app.py" not in paths
    # Docs still included
    assert temp_repo / "README.md" in paths


def test_gather_files_exclude_directory(temp_repo):
    """Exclude patterns can match directories."""
    results = gather_files([".", "src/app.py"], temp_repo, exclude=["src/*"])
    paths = [p for p, _ in results]

    assert temp_repo / "main.py" in paths
    assert temp_repo / "src" / "app.py" not in paths
    assert temp_repo / "src" / "README.md" not in paths


def test_gather_files_exclude_directory_name(temp_repo):
    """Exclude patterns match directories without globbing."""
    results = gather_files(["."], temp_repo, exclude=["src"])
    paths = [p for p, _ in results]

    assert temp_repo / "main.py" in paths
    assert temp_repo / "src" / "app.py" not in paths
    assert temp_repo / "src" / "README.md" not in paths


def test_gather_files_exclude_glob_root_only(temp_repo):
    """Exclude *.md only matches root level (Path.glob semantics)."""
    results = gather_files(["."], temp_repo, exclude=["*.md"])
    paths = [p for p, _ in results]

    assert temp_repo / "main.py" in paths
    # *.md matches root only
    assert temp_repo / "README.md" not in paths
    assert temp_repo / "CONTRIBUTING.md" not in paths
    # Subdirectory .md files NOT excluded by *.md
    assert temp_repo / "src" / "README.md" in paths


def test_gather_files_exclude_glob_recursive(temp_repo):
    """Exclude **/*.md matches all levels (Path.glob semantics)."""
    results = gather_files(["."], temp_repo, exclude=["**/*.md"])
    paths = [p for p, _ in results]

    assert temp_repo / "main.py" in paths
    # **/*.md matches all levels
    assert temp_repo / "README.md" not in paths
    assert temp_repo / "CONTRIBUTING.md" not in paths
    assert temp_repo / "src" / "README.md" not in paths


def test_gather_files_exclude_multiple_patterns(temp_repo):
    """Multiple exclude patterns all apply."""
    results = gather_files(["."], temp_repo, exclude=["**/*.py", "**/*.md"])
    paths = [p for p, _ in results]

    # Both .py and .md excluded at all levels
    assert temp_repo / "main.py" not in paths
    assert temp_repo / "README.md" not in paths
    assert temp_repo / "src" / "app.py" not in paths


def test_gather_files_exclude_overrides_include(temp_repo):
    """Exclude patterns take precedence over explicit includes."""
    # Explicitly request main.py but also exclude it
    results = gather_files(["main.py"], temp_repo, exclude=["main.py"])
    paths = [p for p, _ in results]

    # Exclude wins - main.py should not be included
    assert temp_repo / "main.py" not in paths


def test_gather_files_exclude_overrides_glob_include(temp_repo):
    """Exclude patterns filter out files matched by include globs."""
    # Include all .py files but exclude main.py
    results = gather_files(["**/*.py"], temp_repo, exclude=["main.py"])
    paths = [p for p, _ in results]

    # main.py excluded, src/app.py included
    assert temp_repo / "main.py" not in paths
    assert temp_repo / "src" / "app.py" in paths


# --- Glob semantics consistency tests ---


def test_include_star_md_root_only(temp_repo):
    """Include *.md only matches root level."""
    results = gather_files(["*.md"], temp_repo)
    paths = [p for p, _ in results]

    assert temp_repo / "README.md" in paths
    assert temp_repo / "CONTRIBUTING.md" in paths
    # src/README.md not directly matched by *.md
    # (but may appear as parent doc if other src files included)
    src_readme_direct = any(
        p == temp_repo / "src" / "README.md"
        for p, _ in results
        if "src" in str(p) and p.name == "README.md"
    )
    # Filter to just the files matched by the pattern itself (not parent docs)
    # *.md should only match root level
    root_mds = [p for p in paths if p.parent == temp_repo and p.suffix == ".md"]
    assert len(root_mds) == 2  # README.md and CONTRIBUTING.md


def test_include_doublestar_md_all_levels(temp_repo):
    """Include **/*.md matches all levels."""
    results = gather_files(["**/*.md"], temp_repo)
    paths = [p for p, _ in results]

    assert temp_repo / "README.md" in paths
    assert temp_repo / "CONTRIBUTING.md" in paths
    assert temp_repo / "src" / "README.md" in paths


def test_exclude_star_md_root_only(temp_repo):
    """Exclude *.md only excludes root level."""
    results = gather_files(["."], temp_repo, exclude=["*.md"])
    paths = [p for p, _ in results]

    # Root .md files excluded
    assert temp_repo / "README.md" not in paths
    assert temp_repo / "CONTRIBUTING.md" not in paths
    # Subdirectory .md NOT excluded
    assert temp_repo / "src" / "README.md" in paths


def test_exclude_doublestar_md_all_levels(temp_repo):
    """Exclude **/*.md excludes all levels."""
    results = gather_files(["."], temp_repo, exclude=["**/*.md"])
    paths = [p for p, _ in results]

    # All .md files excluded
    assert temp_repo / "README.md" not in paths
    assert temp_repo / "CONTRIBUTING.md" not in paths
    assert temp_repo / "src" / "README.md" not in paths


# --- Binary file detection tests ---


def test_is_binary_by_extension(temp_repo):
    """is_binary returns True for known binary extensions."""
    png_file = temp_repo / "image.png"
    png_file.write_bytes(b"fake png data")

    assert is_binary(png_file) is True


def test_is_binary_by_content(temp_repo):
    """is_binary returns True for files with null bytes."""
    binary_file = temp_repo / "data.bin"
    binary_file.write_bytes(b"some\x00data")

    assert is_binary(binary_file) is True


def test_is_binary_text_file(temp_repo):
    """is_binary returns False for text files."""
    text_file = temp_repo / "text.txt"
    text_file.write_text("hello world")

    assert is_binary(text_file) is False


def test_is_binary_unreadable_file(temp_repo):
    """is_binary returns True for unreadable files."""
    import os
    unreadable = temp_repo / "unreadable.txt"
    unreadable.write_text("content")
    os.chmod(unreadable, 0o000)

    try:
        assert is_binary(unreadable) is True
    finally:
        os.chmod(unreadable, 0o644)


def test_gather_files_skips_binary_by_extension(temp_repo):
    """gather_files excludes binary files by extension."""
    png_file = temp_repo / "image.png"
    png_file.write_bytes(b"\x89PNG\r\n\x1a\n")  # PNG header
    (temp_repo / "text.txt").write_text("hello")

    results = gather_files(["image.png", "text.txt"], temp_repo)
    paths = [p for p, _ in results]

    assert temp_repo / "text.txt" in paths
    assert temp_repo / "image.png" not in paths


def test_gather_files_skips_binary_by_content(temp_repo):
    """gather_files excludes binary files by content sniffing."""
    binary_file = temp_repo / "unknown.data"
    binary_file.write_bytes(b"content\x00with\x00nulls")
    (temp_repo / "text.txt").write_text("hello")

    results = gather_files(["unknown.data", "text.txt"], temp_repo)
    paths = [p for p, _ in results]

    assert temp_repo / "text.txt" in paths
    assert temp_repo / "unknown.data" not in paths


def test_gather_files_includes_text_formats(temp_repo):
    """gather_files includes common text formats like .json, .lock, .svg."""
    (temp_repo / "data.json").write_text('{"key": "value"}')
    (temp_repo / "uv.lock").write_text("# Lock file\n")
    (temp_repo / "icon.svg").write_text('<svg xmlns="http://www.w3.org/2000/svg"></svg>')

    results = gather_files(["data.json", "uv.lock", "icon.svg"], temp_repo)
    paths = [p for p, _ in results]

    assert temp_repo / "data.json" in paths
    assert temp_repo / "uv.lock" in paths
    assert temp_repo / "icon.svg" in paths


def test_gather_files_glob_with_binary_files(temp_repo):
    """gather_files with glob patterns excludes binary files."""
    (temp_repo / "image.png").write_bytes(b"\x89PNG")
    (temp_repo / "doc.pdf").write_bytes(b"%PDF-1.4")
    (temp_repo / "code.py").write_text("print('hello')")

    results = gather_files(["."], temp_repo)
    paths = [p for p, _ in results]

    assert temp_repo / "code.py" in paths
    assert temp_repo / "image.png" not in paths
    assert temp_repo / "doc.pdf" not in paths
