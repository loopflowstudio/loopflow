"""Shared test payloads for Python tests."""

from __future__ import annotations

WAVE_MINIMAL = {
    "id": "abc-123",
    "object": "wave",
    "name": "reduce",
    "goal": "ship-roadmap",
    "metrics": [],
    "workers": 1,
    "direction": ["infra"],
    "area": [],
    "status": "running",
    "flow_steps": [],
    "parent_wave_id": None,
    "repo": "/tmp/repo",
    "iteration": 0,
}

WAVE_FULL = {
    **WAVE_MINIMAL,
    "created_at": "2026-02-08T01:56:54Z",
    "flow_steps": ["review", "iterate", "build", "gate"],
    "agent": "codex",
    "skill_agents": {"gate": "claude"},
    "active_run": {
        "id": "run-1",
        "wave_id": "abc-123",
        "iteration": 0,
        "step_index": 0,
        "status": "running",
        "local_worktree": "/tmp/wt",
        "remote_branch": "wave/architecture",
        "pr": {
            "url": "https://github.com/org/repo/pull/1",
            "number": 1,
            "state": "open",
        },
        "started_at": "2026-02-08T02:00:00Z",
        "flow_parents": ["parent-1"],
    },
}

WAVE_RUN_MINIMAL = {
    "id": "run-1",
    "wave_id": "abc-123",
    "iteration": 0,
    "step_index": 0,
    "status": "completed",
    "local_worktree": "/tmp/wt",
    "remote_branch": "wave/architecture",
}

REPO_MINIMAL = {
    "path": "/tmp/repo",
    "name": "repo",
    "repo_id": "loopflowstudio/repo",
    "wave_count": 3,
    "registered": True,
    "added_at": "2026-02-24T12:00:00Z",
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
