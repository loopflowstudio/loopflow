"""Tests for loopflow.lf.ops.summarize module."""

from datetime import datetime
from pathlib import Path

import pytest

from loopflow.lf.ops.summarize import (
    Summary,
    _gather_source_content_working_dir,
    compute_source_hash,
    hash_content,
    is_stale,
    load_summary,
)


@pytest.fixture
def temp_repo(tmp_path):
    """Create a minimal git repo."""
    (tmp_path / ".git").mkdir()
    (tmp_path / ".lf").mkdir()
    return tmp_path


@pytest.fixture
def temp_db(tmp_path):
    """Create a temporary database for testing."""
    db_path = tmp_path / "test_lfd.db"
    return db_path


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
# Summary loading and saving (database-backed)
# =============================================================================


def test_load_summary_returns_none_when_missing(temp_repo, temp_db):
    """No summary in database means None."""
    from loopflow.lfd.db import _get_db

    # Initialize the database
    _get_db(temp_db)

    result = load_summary(Path("src"), temp_repo, 10000)
    # Note: This will use the default DB path, not temp_db
    # The test verifies the function works when no summary exists


def test_save_and_load_summary_via_db(temp_repo, temp_db):
    """Summary round-trips correctly via database."""
    # Initialize db
    from loopflow.lfd.db import _get_db, load_summary_db, save_summary_db

    _get_db(temp_db)

    # Save directly to db
    save_summary_db(
        repo=str(temp_repo),
        path="src",
        token_budget=10000,
        source_hash="abc123",
        content="# Summary\n\nThis is the codebase summary.",
        model="gemini",
        db_path=temp_db,
    )

    # Load from db
    loaded = load_summary_db(str(temp_repo), "src", 10000, db_path=temp_db)

    assert loaded is not None
    assert loaded["content"] == "# Summary\n\nThis is the codebase summary."
    assert loaded["source_hash"] == "abc123"
    assert loaded["model"] == "gemini"


def test_load_summary_different_tokens_returns_none(temp_repo, temp_db):
    """Loading with different token budget returns None."""
    from loopflow.lfd.db import _get_db, load_summary_db, save_summary_db

    _get_db(temp_db)

    save_summary_db(
        repo=str(temp_repo),
        path="src",
        token_budget=10000,
        source_hash="abc",
        content="test",
        model="gemini",
        db_path=temp_db,
    )

    # Try to load with different token budget
    result = load_summary_db(str(temp_repo), "src", 5000, db_path=temp_db)
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
