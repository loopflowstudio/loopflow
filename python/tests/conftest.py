"""Shared fixtures for Python tests."""

from __future__ import annotations

import pytest


WAVE_MINIMAL = {
    "id": "abc-123",
    "name": "reduce",
    "repo": "/tmp/repo",
    "flow": "reduce",
    "direction": ["infra-engineer"],
    "area": [],
    "status": "running",
    "iteration": 0,
}

WAVE_FULL = {
    **WAVE_MINIMAL,
    "stimulus": {"kind": "manual"},
    "created_at": "2026-02-08T01:56:54Z",
    "flow_steps": ["review", "iterate", "ship", "gate"],
    "commits": [
        {"sha": "a1b2c3d", "message": "implement: add retry logic"},
        {"sha": "e4f5g6h", "message": "design: initial sketch"},
    ],
    "diff_stat": " 3 files changed, 42 insertions(+), 7 deletions(-)",
    "active_run": {
        "id": "run-1",
        "wave_id": "abc-123",
        "iteration": 0,
        "step_index": 0,
        "status": "running",
        "local_worktree": "/tmp/wt",
        "remote_branch": "wave/reduce",
        "pr": {"url": "https://github.com/org/repo/pull/1", "number": 1, "state": "open"},
        "started_at": "2026-02-08T02:00:00Z",
        "flow_parents": ["parent-1"],
    },
    "branch": "wave/reduce",
    "pr_url": "https://github.com/org/repo/pull/1",
    "pr_state": "open",
}

WAVE_RUN_MINIMAL = {
    "id": "run-1",
    "wave_id": "abc-123",
    "iteration": 0,
    "step_index": 0,
    "status": "completed",
    "local_worktree": "/tmp/wt",
    "remote_branch": "wave/reduce",
}


@pytest.fixture
def wave_minimal():
    return WAVE_MINIMAL.copy()


@pytest.fixture
def wave_full():
    return WAVE_FULL.copy()


@pytest.fixture
def wave_run_minimal():
    return WAVE_RUN_MINIMAL.copy()
