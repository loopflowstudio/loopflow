"""Shared test payloads for Python tests."""

from __future__ import annotations

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
    "stimuli": [{"id": "stim-1", "kind": "loop"}],
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

SESSION_MINIMAL = {
    "id": "session-1",
    "object": "session",
    "harness": "claude",
    "status": "active",
    "config": {},
}

SESSION_FULL = {
    **SESSION_MINIMAL,
    "wave_run_id": "run-1",
    "provider_session_id": "provider-1",
    "config": {
        "model": "claude-sonnet-4-5-20250929",
        "cwd": "/tmp/repo",
        "system_prompt": "be concise",
        "max_turns": 3,
        "yolo_mode": True,
    },
    "created_at": "2026-02-24T12:00:00Z",
    "ended_at": "2026-02-24T12:05:00Z",
}
