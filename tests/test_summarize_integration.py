"""Integration tests for summarize flow with context module."""

import os
from datetime import datetime
from pathlib import Path
from unittest.mock import patch

import pytest

from loopflow.lf.config import Config, SummaryConfig
from loopflow.lf.context import _trigger_background_refresh, gather_summaries
from loopflow.lfd.db import _get_db, save_summary_db
from loopflow.lfops.summarize import (
    Summary,
    compute_source_hash,
    pathset_hash,
    pathset_key,
)


@pytest.fixture
def temp_repo(tmp_path):
    """Create a minimal git repo with config."""
    (tmp_path / ".git").mkdir()
    (tmp_path / ".lf").mkdir()
    (tmp_path / "src").mkdir()
    (tmp_path / "src" / "main.py").write_text("print('hello')")
    return tmp_path


@pytest.fixture
def temp_db(tmp_path):
    """Create a temporary database."""
    db_path = tmp_path / "test_lfd.db"
    _get_db(db_path)
    return db_path


@pytest.fixture
def temp_lf_dir(tmp_path):
    """Create a temporary ~/.lf directory."""
    lf_dir = tmp_path / ".lf"
    lf_dir.mkdir(exist_ok=True)
    return lf_dir


# =============================================================================
# gather_summaries tests
# =============================================================================


def test_gather_summaries_returns_empty_when_no_config(temp_repo):
    """No summaries config means empty list."""
    config = Config()
    result = gather_summaries(temp_repo, config)
    assert result == []


def test_gather_summaries_returns_empty_when_none_config(temp_repo):
    """None config means empty list."""
    result = gather_summaries(temp_repo, None)
    assert result == []


def test_gather_summaries_loads_from_db(temp_repo, temp_db):
    """Loads cached summary from database."""
    # Save summary to db
    source_hash = compute_source_hash(Path("src"), temp_repo)
    save_summary_db(
        repo=str(temp_repo),
        path="src",
        token_budget=10000,
        source_hash=source_hash,
        content="# Summary of src",
        model="gemini",
        db_path=temp_db,
    )

    # Create config
    config = Config(
        summaries=[SummaryConfig(path="src")],
        summary_tokens=10000,
    )

    # Mock load_summary to use our temp db
    with patch("loopflow.lf.context.load_summary") as mock_load:
        mock_load.return_value = Summary(
            path=Path("src"),
            content="# Summary of src",
            token_budget=10000,
            source_hash=source_hash,
            created_at=datetime.now(),
            model="gemini",
        )

        with patch("loopflow.lf.context._trigger_background_refresh"):
            result = gather_summaries(temp_repo, config)

    assert len(result) == 1
    assert result[0][0] == Path("src")
    assert result[0][1] == "# Summary of src"


def test_gather_summaries_triggers_refresh_when_missing(temp_repo):
    """Triggers background refresh when summary not in db."""
    config = Config(
        summaries=[SummaryConfig(path="src")],
        summary_tokens=10000,
    )

    with patch("loopflow.lf.context.load_summary", return_value=None):
        with patch("loopflow.lf.context._trigger_background_refresh") as mock_refresh:
            result = gather_summaries(temp_repo, config)

    assert result == []
    mock_refresh.assert_called_once_with(temp_repo)


def test_gather_summaries_triggers_refresh_when_stale(temp_repo, temp_db):
    """Triggers background refresh when summary is stale."""
    config = Config(
        summaries=[SummaryConfig(path="src")],
        summary_tokens=10000,
    )

    with patch("loopflow.lf.context.load_summary") as mock_load:
        # Return summary with old hash
        mock_load.return_value = Summary(
            path=Path("src"),
            content="# Old summary",
            token_budget=10000,
            source_hash="old_hash_that_doesnt_match",
            created_at=datetime.now(),
            model="gemini",
        )

        with patch("loopflow.lf.context._trigger_background_refresh") as mock_refresh:
            result = gather_summaries(temp_repo, config)

    # Still returns the stale summary
    assert len(result) == 1
    assert result[0][1] == "# Old summary"
    # But triggers refresh
    mock_refresh.assert_called_once_with(temp_repo)


# =============================================================================
# _trigger_background_refresh tests
# =============================================================================


def test_trigger_background_refresh_creates_lock(temp_repo, temp_lf_dir):
    """Creates lock file with PID when spawning refresh."""
    lock_file = temp_lf_dir / ".refresh.lock"

    with patch.object(Path, "home", return_value=temp_lf_dir.parent):
        with patch("loopflow.lf.context.subprocess.Popen") as mock_popen:
            mock_popen.return_value.pid = 12345
            _trigger_background_refresh(temp_repo)

    assert lock_file.exists()
    assert lock_file.read_text() == "12345"


def test_trigger_background_refresh_skips_if_locked(temp_repo, temp_lf_dir):
    """Skips refresh if lock exists with running process."""
    lock_file = temp_lf_dir / ".refresh.lock"
    # Use current process PID (which is definitely running)
    lock_file.write_text(str(os.getpid()))

    with patch.object(Path, "home", return_value=temp_lf_dir.parent):
        with patch("loopflow.lf.context.subprocess.Popen") as mock_popen:
            _trigger_background_refresh(temp_repo)

    # Should not spawn new process
    mock_popen.assert_not_called()


def test_trigger_background_refresh_cleans_stale_lock(temp_repo, temp_lf_dir):
    """Removes stale lock and proceeds with refresh."""
    lock_file = temp_lf_dir / ".refresh.lock"
    # Use a PID that definitely doesn't exist
    lock_file.write_text("999999999")

    with patch.object(Path, "home", return_value=temp_lf_dir.parent):
        with patch("loopflow.lf.context.subprocess.Popen") as mock_popen:
            mock_popen.return_value.pid = 12345
            _trigger_background_refresh(temp_repo)

    # Should spawn new process
    mock_popen.assert_called_once()
    # Lock should have new PID
    assert lock_file.read_text() == "12345"


def test_trigger_background_refresh_cleans_invalid_lock(temp_repo, temp_lf_dir):
    """Removes lock with invalid content and proceeds."""
    lock_file = temp_lf_dir / ".refresh.lock"
    lock_file.write_text("not_a_pid")

    with patch.object(Path, "home", return_value=temp_lf_dir.parent):
        with patch("loopflow.lf.context.subprocess.Popen") as mock_popen:
            mock_popen.return_value.pid = 12345
            _trigger_background_refresh(temp_repo)

    mock_popen.assert_called_once()
    assert lock_file.read_text() == "12345"


# =============================================================================
# pathset tests
# =============================================================================


def test_pathset_key_single_path():
    """Single path returns just that path as string."""
    result = pathset_key([Path("src")])
    assert result == "src"


def test_pathset_key_multiple_paths_sorted():
    """Multiple paths are sorted and comma-joined."""
    result = pathset_key([Path("tests"), Path("swift"), Path("docs")])
    assert result == "docs,swift,tests"


def test_pathset_key_already_sorted():
    """Already sorted paths work correctly."""
    result = pathset_key([Path("a"), Path("b"), Path("c")])
    assert result == "a,b,c"


def test_pathset_key_nested_paths():
    """Nested paths are handled correctly."""
    result = pathset_key([Path("src/loopflow"), Path("src/lfd")])
    assert result == "src/lfd,src/loopflow"


def test_pathset_hash_single_hash():
    """Single hash returns a valid hash."""
    result = pathset_hash(["abc123"])
    assert len(result) == 16  # hash_content returns 16 chars


def test_pathset_hash_multiple_hashes():
    """Multiple hashes combine deterministically."""
    result1 = pathset_hash(["hash1", "hash2", "hash3"])
    result2 = pathset_hash(["hash1", "hash2", "hash3"])
    assert result1 == result2


def test_pathset_hash_order_independent():
    """Hash order doesn't matter (sorted internally)."""
    result1 = pathset_hash(["hash1", "hash2", "hash3"])
    result2 = pathset_hash(["hash3", "hash1", "hash2"])
    assert result1 == result2


def test_pathset_hash_different_inputs_different_output():
    """Different inputs produce different hashes."""
    result1 = pathset_hash(["hash1", "hash2"])
    result2 = pathset_hash(["hash1", "hash3"])
    assert result1 != result2


def test_pathset_caching_saves_under_pathset_key(temp_repo, temp_db):
    """Pathset summaries are saved with comma-joined key."""
    from loopflow.lfd.db import load_summary_db

    # Save a summary under a pathset key
    pathset = "docs,swift,tests"
    combined_hash = pathset_hash(["hash1", "hash2", "hash3"])

    save_summary_db(
        repo=str(temp_repo),
        path=pathset,
        token_budget=10000,
        source_hash=combined_hash,
        content="# Combined summary of docs, swift, tests",
        model="claude:sonnet",
        db_path=temp_db,
    )

    # Load it back
    loaded = load_summary_db(str(temp_repo), pathset, 10000, temp_db)

    assert loaded is not None
    assert loaded["content"] == "# Combined summary of docs, swift, tests"
    assert loaded["source_hash"] == combined_hash


def test_pathset_cache_hit_with_matching_hash(temp_repo, temp_db):
    """Cache hit when pathset hash matches."""

    from loopflow.lfops.summarize import load_summary

    pathset = "docs,swift"
    combined_hash = pathset_hash(["aaa", "bbb"])

    # Save summary
    save_summary_db(
        repo=str(temp_repo),
        path=pathset,
        token_budget=5000,
        source_hash=combined_hash,
        content="# Cached pathset summary",
        model="test",
        db_path=temp_db,
    )

    # Load and verify
    with patch("loopflow.lfd.db.DB_PATH", temp_db):
        loaded = load_summary(Path(pathset), temp_repo, 5000)

    assert loaded is not None
    assert loaded.content == "# Cached pathset summary"
    assert loaded.source_hash == combined_hash


def test_pathset_cache_miss_with_different_hash(temp_repo, temp_db):
    """Cache miss when pathset hash doesn't match (stale)."""
    from loopflow.lfops.summarize import load_summary

    pathset = "docs,swift"
    old_hash = pathset_hash(["old1", "old2"])
    new_hash = pathset_hash(["new1", "new2"])

    # Save with old hash
    save_summary_db(
        repo=str(temp_repo),
        path=pathset,
        token_budget=5000,
        source_hash=old_hash,
        content="# Old summary",
        model="test",
        db_path=temp_db,
    )

    # Load
    with patch("loopflow.lfd.db.DB_PATH", temp_db):
        loaded = load_summary(Path(pathset), temp_repo, 5000)

    # Summary loads but hash won't match new_hash
    assert loaded is not None
    assert loaded.source_hash == old_hash
    assert loaded.source_hash != new_hash  # Stale!


# =============================================================================
# merge-base hashing tests
# =============================================================================


@pytest.fixture
def git_repo(tmp_path):
    """Create a real git repo with main branch and commits."""
    import subprocess

    repo = tmp_path / "repo"
    repo.mkdir()

    # Init and configure
    subprocess.run(["git", "init"], cwd=repo, capture_output=True)
    subprocess.run(["git", "config", "user.email", "test@test.com"], cwd=repo, capture_output=True)
    subprocess.run(["git", "config", "user.name", "Test"], cwd=repo, capture_output=True)

    # Create initial structure on main
    (repo / "src").mkdir()
    (repo / "src" / "main.py").write_text("print('v1')")
    (repo / "tests").mkdir()
    (repo / "tests" / "test_main.py").write_text("def test_v1(): pass")

    subprocess.run(["git", "add", "."], cwd=repo, capture_output=True)
    subprocess.run(["git", "commit", "-m", "initial"], cwd=repo, capture_output=True)

    # Rename branch to main (in case default is master)
    subprocess.run(["git", "branch", "-M", "main"], cwd=repo, capture_output=True)

    return repo


def test_hash_uses_merge_base_not_head(git_repo):
    """Hash is computed at merge-base with main, not at HEAD."""
    import subprocess

    from loopflow.lfops.summarize import compute_source_hash

    # Get hash on main
    hash_on_main = compute_source_hash(Path("src"), git_repo)

    # Create feature branch
    subprocess.run(["git", "checkout", "-b", "feature"], cwd=git_repo, capture_output=True)

    # Modify file on branch
    (git_repo / "src" / "main.py").write_text("print('v2 - modified on branch')")
    subprocess.run(["git", "add", "."], cwd=git_repo, capture_output=True)
    subprocess.run(["git", "commit", "-m", "branch change"], cwd=git_repo, capture_output=True)

    # Hash should still match main (merge-base unchanged)
    hash_on_branch = compute_source_hash(Path("src"), git_repo)
    assert hash_on_branch == hash_on_main


def test_hash_changes_when_main_advances(git_repo):
    """Hash changes when main branch advances."""
    import subprocess

    from loopflow.lfops.summarize import compute_source_hash

    # Create feature branch from main
    subprocess.run(["git", "checkout", "-b", "feature"], cwd=git_repo, capture_output=True)

    # Get initial hash
    hash_before = compute_source_hash(Path("src"), git_repo)

    # Go back to main and make a commit
    subprocess.run(["git", "checkout", "main"], cwd=git_repo, capture_output=True)
    (git_repo / "src" / "main.py").write_text("print('v2 - new on main')")
    subprocess.run(["git", "add", "."], cwd=git_repo, capture_output=True)
    subprocess.run(["git", "commit", "-m", "main advance"], cwd=git_repo, capture_output=True)

    # Go back to feature and rebase
    subprocess.run(["git", "checkout", "feature"], cwd=git_repo, capture_output=True)
    subprocess.run(["git", "rebase", "main"], cwd=git_repo, capture_output=True)

    # Now merge-base has advanced, hash should change
    hash_after = compute_source_hash(Path("src"), git_repo)
    assert hash_after != hash_before


def test_hash_unchanged_by_untracked_files(git_repo):
    """Untracked files don't affect the hash."""
    import subprocess

    from loopflow.lfops.summarize import compute_source_hash

    # Create feature branch
    subprocess.run(["git", "checkout", "-b", "feature"], cwd=git_repo, capture_output=True)

    hash_before = compute_source_hash(Path("src"), git_repo)

    # Add untracked file
    (git_repo / "src" / "untracked.py").write_text("# not committed")

    hash_after = compute_source_hash(Path("src"), git_repo)
    assert hash_after == hash_before


def test_hash_unchanged_by_staged_changes(git_repo):
    """Staged but uncommitted changes don't affect the hash."""
    import subprocess

    from loopflow.lfops.summarize import compute_source_hash

    subprocess.run(["git", "checkout", "-b", "feature"], cwd=git_repo, capture_output=True)

    hash_before = compute_source_hash(Path("src"), git_repo)

    # Stage a change but don't commit
    (git_repo / "src" / "main.py").write_text("print('staged change')")
    subprocess.run(["git", "add", "src/main.py"], cwd=git_repo, capture_output=True)

    hash_after = compute_source_hash(Path("src"), git_repo)
    assert hash_after == hash_before


def test_subdir_hashes_independent(git_repo):
    """Each subdir has independent hash based on its merge-base content."""
    import subprocess

    from loopflow.lfops.summarize import compute_subdir_hashes

    subprocess.run(["git", "checkout", "-b", "feature"], cwd=git_repo, capture_output=True)

    hashes = compute_subdir_hashes(Path("."), git_repo)

    # Should have hashes for src and tests
    assert "src" in hashes
    assert "tests" in hashes
    # They should be different (different content)
    assert hashes["src"] != hashes["tests"]


def test_on_main_branch_hash_still_works(git_repo):
    """Hash computation works when already on main (merge-base is HEAD)."""
    from loopflow.lfops.summarize import compute_source_hash

    # Stay on main
    hash_on_main = compute_source_hash(Path("src"), git_repo)

    # Should get a valid hash
    assert hash_on_main is not None
    assert len(hash_on_main) == 16  # hash_content returns 16 chars
