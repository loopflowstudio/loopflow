"""Integration tests for summarize flow with context module."""

import os
from pathlib import Path
from unittest.mock import patch

import pytest

from loopflow.lf.config import Config, SummaryConfig
from loopflow.lf.context import _trigger_background_refresh, gather_summaries
from loopflow.lfd.db import _get_db, save_summary_db
from loopflow.lfops.summarize import compute_source_hash


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
        from loopflow.lfops.summarize import Summary
        from datetime import datetime

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
        from loopflow.lfops.summarize import Summary
        from datetime import datetime

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
