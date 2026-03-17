"""Shared test payloads for Python tests."""

from __future__ import annotations

WAVE_MINIMAL = {
    "id": "abc-123",
    "name": "reduce",
    "repo": "/tmp/repo",
    "primary_flow": "reduce",
    "direction": ["infra"],
    "area": [],
    "status": "running",
    "iteration": 0,
}

WAVE_FULL = {
    **WAVE_MINIMAL,
    "triggers": [{"id": "trig-1", "signal": "repo"}],
    "created_at": "2026-02-08T01:56:54Z",
    "flow_steps": ["review", "iterate", "build", "gate"],
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

REPO_MINIMAL = {
    "path": "/tmp/repo",
    "name": "repo",
    "repo_id": "loopflowstudio/repo",
    "wave_count": 3,
    "registered": True,
    "added_at": "2026-02-24T12:00:00Z",
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
        "agent": "claude-sonnet-4-5-20250929",
        "cwd": "/tmp/repo",
        "system_prompt": "be concise",
        "max_turns": 3,
        "yolo_mode": True,
    },
    "created_at": "2026-02-24T12:00:00Z",
    "ended_at": "2026-02-24T12:05:00Z",
}

AUTH_PROVIDER_ACTIVE = {
    "provider": "github",
    "status": "active",
    "login": "jackdanger",
    "credential_type": "oauth",
}

AUTH_PROVIDER_ACTIVE_WITH_TIMESTAMPS = {
    "provider": "claude",
    "status": "active",
    "login": "jack@anthropic.com",
    "expires_at": "2030-01-01T04:00:00Z",
    "next_refresh_at": "2030-01-01T03:40:00Z",
    "credential_type": "oauth",
}

AUTH_PROVIDER_APIKEY = {
    "provider": "codex",
    "status": "active",
    "credential_type": "apikey",
}

AUTH_PROVIDER_NONE = {
    "provider": "opencodezen",
    "status": "none",
}

AUTH_FLOW = {
    "provider": "github",
    "verification_uri": "https://github.com/login/device",
    "verification_uri_complete": "https://github.com/login/device?user_code=ABCD-1234",
    "user_code": "ABCD-1234",
    "expires_in": 900,
}

PROVIDER_INFO_MINIMAL = {
    "provider": "codex",
    "auth_status": "none",
    "billing": "subscription",
    "models": [
        {
            "id": "gpt-5.1-codex",
            "display_name": "GPT-5.1 Codex",
            "provider": "codex",
        }
    ],
}

PROVIDER_INFO_FULL = {
    "provider": "opencodezen",
    "auth_status": "active",
    "login": "user@example.com",
    "billing": "per_token",
    "models": [
        {
            "id": "opencode/kimi-k2.5",
            "display_name": "Kimi K2.5",
            "provider": "opencodezen",
            "cost_rates": {
                "input_per_mtok": 0.5,
                "output_per_mtok": 1.0,
                "cache_read_per_mtok": None,
                "cache_write_per_mtok": None,
            },
        }
    ],
}

USAGE_SUMMARY_BY_MODEL = {
    "object": "usage_summary",
    "group_by": "model",
    "from": None,
    "to": None,
    "groups": [
        {
            "key": "gpt-5.1-codex",
            "tokens": {
                "input": 42100,
                "output": 8500,
                "reasoning": 0,
                "cache_read": 0,
                "cache_write": 0,
            },
            "sessions": 3,
            "turns": 45,
        },
        {
            "key": "opencode/kimi-k2.5",
            "tokens": {
                "input": 1_000_000,
                "output": 500_000,
                "reasoning": 0,
                "cache_read": 0,
                "cache_write": 0,
            },
            "sessions": 8,
            "turns": 120,
        },
    ],
}

USAGE_SUMMARY = {
    "object": "usage_summary",
    "group_by": "wave",
    "from": None,
    "to": None,
    "groups": [
        {
            "key": "engbot",
            "tokens": {
                "input": 42100,
                "output": 8500,
                "reasoning": 0,
                "cache_read": 0,
                "cache_write": 0,
            },
            "sessions": 3,
            "turns": 45,
        },
        {
            "key": "infra",
            "tokens": {
                "input": 128300,
                "output": 24100,
                "reasoning": 12000,
                "cache_read": 45000,
                "cache_write": 3200,
            },
            "sessions": 8,
            "turns": 120,
        },
    ],
}
