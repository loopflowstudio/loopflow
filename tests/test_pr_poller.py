"""Tests for pr_poller module."""

import time
from pathlib import Path

from loopflow.lfd.pr_poller import (
    INTERVAL_INITIAL,
    INTERVAL_PENDING,
    INTERVAL_STABLE,
    PRPoller,
    PRState,
    _get_poll_interval,
)


def test_poll_interval_pending_is_fast():
    """Pending CI state uses fast polling interval."""
    interval = _get_poll_interval("PENDING", "OPEN")
    assert interval == INTERVAL_PENDING


def test_poll_interval_stable_is_slow():
    """Stable CI states use slow polling interval."""
    assert _get_poll_interval("SUCCESS", "OPEN") == INTERVAL_STABLE
    assert _get_poll_interval("FAILURE", "OPEN") == INTERVAL_STABLE
    assert _get_poll_interval(None, "OPEN") == INTERVAL_STABLE


def test_poll_interval_merged_is_infinite():
    """Merged/closed PRs stop polling."""
    interval = _get_poll_interval("SUCCESS", "MERGED")
    assert interval == float("inf")

    interval = _get_poll_interval("PENDING", "CLOSED")
    assert interval == float("inf")


def test_pr_state_defaults():
    """PRState has correct defaults."""
    state = PRState(repo=Path("/tmp/repo"), branch="feature", pr_number=123)

    assert state.ci_state is None
    assert state.pr_state == "OPEN"
    assert state.last_poll == 0
    assert state.next_poll == 0


def test_poller_track_creates_state():
    """track() creates PRState with initial poll time."""
    poller = PRPoller()
    now = time.time()

    poller.track(Path("/tmp/repo"), "feature", 123)

    assert poller.is_tracked(Path("/tmp/repo"), "feature")
    states = poller.list_tracked()
    assert len(states) == 1
    assert states[0].branch == "feature"
    assert states[0].pr_number == 123
    assert states[0].next_poll >= now + INTERVAL_INITIAL - 1


def test_poller_untrack_removes_state():
    """untrack() removes PRState."""
    poller = PRPoller()

    poller.track(Path("/tmp/repo"), "feature", 123)
    assert poller.is_tracked(Path("/tmp/repo"), "feature")

    poller.untrack(Path("/tmp/repo"), "feature")
    assert not poller.is_tracked(Path("/tmp/repo"), "feature")


def test_poller_untrack_nonexistent_is_safe():
    """untrack() on nonexistent PR doesn't raise."""
    poller = PRPoller()
    poller.untrack(Path("/tmp/repo"), "nonexistent")  # Should not raise


def test_poller_tracks_multiple_prs():
    """Poller can track multiple PRs from different repos."""
    poller = PRPoller()

    poller.track(Path("/tmp/repo-a"), "feature-1", 1)
    poller.track(Path("/tmp/repo-a"), "feature-2", 2)
    poller.track(Path("/tmp/repo-b"), "feature-1", 3)

    states = poller.list_tracked()
    assert len(states) == 3


def test_poller_key_includes_repo_and_branch():
    """PRs are keyed by both repo and branch."""
    poller = PRPoller()

    # Same branch name in different repos
    poller.track(Path("/tmp/repo-a"), "feature", 1)
    poller.track(Path("/tmp/repo-b"), "feature", 2)

    assert poller.is_tracked(Path("/tmp/repo-a"), "feature")
    assert poller.is_tracked(Path("/tmp/repo-b"), "feature")

    states = poller.list_tracked()
    assert len(states) == 2


def test_pr_state_key_format():
    """PRPoller uses repo:branch as key."""
    poller = PRPoller()
    key = poller._key(Path("/tmp/repo"), "feature")
    assert key == "/tmp/repo:feature"
