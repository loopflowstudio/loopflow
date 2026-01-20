"""Tests for loopflow.lfops.summarize module."""

from datetime import datetime
from pathlib import Path

import pytest

from loopflow.lfops.summarize import (
    Summary,
    SummaryMetadata,
    _ensure_gitignored,
    _gather_source_content_working_dir,
    _load_metadata,
    _path_to_filename,
    _save_metadata,
    compute_source_hash,
    hash_content,
    is_stale,
    load_summary,
    save_summary,
)


@pytest.fixture
def temp_repo(tmp_path):
    """Create a minimal git repo."""
    (tmp_path / ".git").mkdir()
    (tmp_path / ".lf").mkdir()
    return tmp_path


# =============================================================================
# Path to filename conversion
# =============================================================================


def test_path_to_filename_root():
    """Root path becomes 'root-{tokens}.md'."""
    assert _path_to_filename(Path("."), 10000) == "root-10000.md"


def test_path_to_filename_simple():
    """Simple path is converted to filename."""
    assert _path_to_filename(Path("src"), 10000) == "src-10000.md"


def test_path_to_filename_nested():
    """Nested path has slashes replaced with dashes."""
    assert _path_to_filename(Path("src/backend"), 5000) == "src-backend-5000.md"


def test_path_to_filename_different_tokens():
    """Different token budgets produce different filenames."""
    assert _path_to_filename(Path("src"), 10000) != _path_to_filename(Path("src"), 5000)


# =============================================================================
# Metadata loading and saving
# =============================================================================


def test_load_metadata_returns_empty_when_missing(temp_repo):
    """No metadata file means empty dict."""
    assert _load_metadata(temp_repo) == {}


def test_save_and_load_metadata(temp_repo):
    """Metadata round-trips correctly."""
    metadata = {
        "src:10000": SummaryMetadata(
            source_hash="abc123",
            token_budget=10000,
            created_at="2025-01-15T10:00:00",
            model="gemini",
        )
    }

    _save_metadata(temp_repo, metadata)
    loaded = _load_metadata(temp_repo)

    assert "src:10000" in loaded
    assert loaded["src:10000"].source_hash == "abc123"
    assert loaded["src:10000"].token_budget == 10000
    assert loaded["src:10000"].model == "gemini"


def test_save_metadata_creates_directory(tmp_path):
    """Saving metadata creates .lf/summaries/ if needed."""
    repo = tmp_path
    (repo / ".git").mkdir()
    # No .lf directory yet

    metadata = {
        "test:1000": SummaryMetadata(
            source_hash="abc",
            token_budget=1000,
            created_at="2025-01-15T10:00:00",
            model="gemini",
        )
    }

    _save_metadata(repo, metadata)

    assert (repo / ".lf" / "summaries" / "_metadata.json").exists()


# =============================================================================
# Gitignore handling
# =============================================================================


def test_ensure_gitignored_creates_gitignore(temp_repo):
    """Creates .gitignore with summaries pattern if missing."""
    _ensure_gitignored(temp_repo)

    gitignore = temp_repo / ".gitignore"
    assert gitignore.exists()
    assert ".lf/summaries/" in gitignore.read_text()


def test_ensure_gitignored_appends_to_existing(temp_repo):
    """Appends to existing .gitignore."""
    gitignore = temp_repo / ".gitignore"
    gitignore.write_text("*.pyc\n")

    _ensure_gitignored(temp_repo)

    content = gitignore.read_text()
    assert "*.pyc" in content
    assert ".lf/summaries/" in content


def test_ensure_gitignored_adds_newline_if_missing(temp_repo):
    """Adds newline before pattern if file doesn't end with one."""
    gitignore = temp_repo / ".gitignore"
    gitignore.write_text("*.pyc")  # No trailing newline

    _ensure_gitignored(temp_repo)

    content = gitignore.read_text()
    assert content == "*.pyc\n.lf/summaries/\n"


def test_ensure_gitignored_idempotent(temp_repo):
    """Doesn't duplicate pattern if already present."""
    gitignore = temp_repo / ".gitignore"
    gitignore.write_text(".lf/summaries/\n")

    _ensure_gitignored(temp_repo)

    content = gitignore.read_text()
    assert content.count(".lf/summaries/") == 1


# =============================================================================
# Hash functions
# =============================================================================


def test_hash_content_deterministic():
    """Same content produces same hash."""
    assert hash_content("hello") == hash_content("hello")


def test_hash_content_different_for_different_content():
    """Different content produces different hash."""
    assert hash_content("hello") != hash_content("world")


def test_hash_content_returns_16_chars():
    """Hash is truncated to 16 characters."""
    assert len(hash_content("test")) == 16


def test_compute_source_hash_for_file(temp_repo):
    """Computes hash for single file."""
    test_file = temp_repo / "test.py"
    test_file.write_text("print('hello')")

    hash1 = compute_source_hash(Path("test.py"), temp_repo)

    # Same content = same hash
    assert hash1 == hash_content("print('hello')")


def test_compute_source_hash_changes_with_content(temp_repo):
    """Hash changes when file content changes."""
    test_file = temp_repo / "test.py"
    test_file.write_text("print('hello')")
    hash1 = compute_source_hash(Path("test.py"), temp_repo)

    test_file.write_text("print('world')")
    hash2 = compute_source_hash(Path("test.py"), temp_repo)

    assert hash1 != hash2


# =============================================================================
# Summary loading and saving
# =============================================================================


def test_load_summary_returns_none_when_missing(temp_repo):
    """No summary file means None."""
    result = load_summary(Path("src"), temp_repo, 10000)
    assert result is None


def test_save_and_load_summary(temp_repo):
    """Summary round-trips correctly."""
    summary = Summary(
        path=Path("src"),
        content="# Summary\n\nThis is the codebase summary.",
        token_budget=10000,
        source_hash="abc123",
        created_at=datetime(2025, 1, 15, 10, 0, 0),
        model="gemini",
    )

    save_summary(summary, temp_repo)
    loaded = load_summary(Path("src"), temp_repo, 10000)

    assert loaded is not None
    assert loaded.path == Path("src")
    assert loaded.content == "# Summary\n\nThis is the codebase summary."
    assert loaded.token_budget == 10000
    assert loaded.source_hash == "abc123"
    assert loaded.model == "gemini"


def test_save_summary_creates_summaries_dir(tmp_path):
    """Saving summary creates .lf/summaries/ directory."""
    repo = tmp_path
    (repo / ".git").mkdir()

    summary = Summary(
        path=Path("src"),
        content="test",
        token_budget=1000,
        source_hash="abc",
        created_at=datetime.now(),
        model="gemini",
    )

    save_summary(summary, repo)

    assert (repo / ".lf" / "summaries").is_dir()


def test_load_summary_different_tokens_returns_none(temp_repo):
    """Loading with different token budget returns None."""
    summary = Summary(
        path=Path("src"),
        content="test",
        token_budget=10000,
        source_hash="abc",
        created_at=datetime.now(),
        model="gemini",
    )
    save_summary(summary, temp_repo)

    # Try to load with different token budget
    result = load_summary(Path("src"), temp_repo, 5000)
    assert result is None


# =============================================================================
# Staleness detection
# =============================================================================


def test_is_stale_returns_false_when_unchanged(temp_repo):
    """Summary is not stale when source hasn't changed."""
    test_file = temp_repo / "test.py"
    test_file.write_text("print('hello')")

    current_hash = compute_source_hash(Path("test.py"), temp_repo)
    summary = Summary(
        path=Path("test.py"),
        content="summary",
        token_budget=1000,
        source_hash=current_hash,
        created_at=datetime.now(),
        model="gemini",
    )

    assert is_stale(summary, temp_repo) is False


def test_is_stale_returns_true_when_changed(temp_repo):
    """Summary is stale when source has changed."""
    test_file = temp_repo / "test.py"
    test_file.write_text("print('hello')")

    summary = Summary(
        path=Path("test.py"),
        content="summary",
        token_budget=1000,
        source_hash="old_hash_that_no_longer_matches",
        created_at=datetime.now(),
        model="gemini",
    )

    assert is_stale(summary, temp_repo) is True


# =============================================================================
# Content gathering
# =============================================================================


def test_gather_source_content_working_dir_single_file(temp_repo):
    """Gathers content from single file."""
    test_file = temp_repo / "test.py"
    test_file.write_text("print('hello')")

    content = _gather_source_content_working_dir(Path("test.py"), temp_repo)

    assert "test.py" in content
    assert "print('hello')" in content


def test_gather_source_content_working_dir_directory(temp_repo):
    """Gathers content from directory."""
    src = temp_repo / "src"
    src.mkdir()
    (src / "main.py").write_text("def main(): pass")
    (src / "utils.py").write_text("def helper(): pass")

    content = _gather_source_content_working_dir(Path("src"), temp_repo)

    assert "src/main.py" in content
    assert "src/utils.py" in content
    assert "def main(): pass" in content
    assert "def helper(): pass" in content


def test_gather_source_content_working_dir_excludes_patterns(temp_repo):
    """Respects exclude patterns."""
    src = temp_repo / "src"
    src.mkdir()
    (src / "main.py").write_text("production code")
    (src / "main_test.py").write_text("test code")

    # Pattern must use ** for recursive matching
    content = _gather_source_content_working_dir(Path("src"), temp_repo, exclude=["**/*_test.py"])

    assert "main.py" in content
    assert "production code" in content
    assert "test code" not in content


def test_gather_source_content_working_dir_skips_summaries(temp_repo):
    """Doesn't include .lf/summaries/ content."""
    summaries = temp_repo / ".lf" / "summaries"
    summaries.mkdir(parents=True)
    (summaries / "cached.md").write_text("old summary")
    (temp_repo / "src.py").write_text("source")

    content = _gather_source_content_working_dir(Path("."), temp_repo)

    assert "old summary" not in content
    assert "source" in content


# =============================================================================
# Config integration
# =============================================================================


def test_config_summaries_loaded(temp_repo):
    """Summaries config is loaded from config.yaml."""
    from loopflow.lf.config import load_config

    config_yaml = temp_repo / ".lf" / "config.yaml"
    config_yaml.write_text("""
summary_tokens: 25000
summaries:
  - path: src/backend
  - path: src/frontend
    tokens: 5000
    model: claude
""")

    config = load_config(temp_repo)

    assert config is not None
    assert config.summary_tokens == 25000
    assert len(config.summaries) == 2
    assert config.summaries[0].path == "src/backend"
    assert config.summaries[0].tokens is None  # Falls back to summary_tokens
    assert config.summaries[1].path == "src/frontend"
    assert config.summaries[1].tokens == 5000
    assert config.summaries[1].model == "claude"


def test_config_summaries_defaults_empty(temp_repo):
    """Summaries config defaults to empty list."""
    from loopflow.lf.config import load_config

    config_yaml = temp_repo / ".lf" / "config.yaml"
    config_yaml.write_text("yolo: false\n")

    config = load_config(temp_repo)

    assert config is not None
    assert config.summaries == []
    assert config.summary_tokens == 10000  # default


def test_config_summary_tokens_default(temp_repo):
    """summary_tokens defaults to 10000."""
    from loopflow.lf.config import load_config

    config_yaml = temp_repo / ".lf" / "config.yaml"
    config_yaml.write_text("""
summaries:
  - path: src
""")

    config = load_config(temp_repo)

    assert config is not None
    assert config.summary_tokens == 10000
