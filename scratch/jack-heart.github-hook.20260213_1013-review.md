# GitHub CI hook + sidecar auto-fix — Design Review

## What was implemented

GitHub CI failure handling for loopflow waves, covering three integration paths:

1. **Webhook ingestion** (`POST /v0/hooks/github`) — receives `check_run` events from GitHub, verifies HMAC SHA-256 signatures, matches failures to active wave runs by repo/branch/PR, and emits `Event::CiFailure`.

2. **One-shot CI polling** — startup poll (when `github.token` is configured) and per-wave endpoint (`POST /v0/waves/{wave_id}/check-ci`) that queries the GitHub check-runs API for failed checks.

3. **Sidecar CI fix runs** — `Event::CiFailure` triggers spawn ephemeral worktrees (`.ci-fix.<id>`), create `WaveRun` records typed as `Sidecar/CiFix`, execute the debug prompt, commit+push fixes, and clean up.

Supporting changes:
- `WaveRunKind` (Main/Sidecar) and `SidecarKind` (CiFix) enums on `WaveRun` with DB migration `004_wave_run_kind.sql`.
- `WaveRun::is_main()` guard so sidecars don't interfere with "active run" queries.
- `GitHubConfig` on `LfdConfig` with env overrides (`LFD_GITHUB_WEBHOOK_SECRET`, `LFD_GITHUB_TOKEN`).
- In-memory `(wave_id, commit_sha)` dedup cache for CI failure events.
- Unified startup worktree janitor handling both fork and sidecar ephemeral worktrees.
- Executor module consolidation (4 files → 1 file).

## Key choices

| Decision | Rationale | Alternative considered |
|----------|-----------|----------------------|
| In-memory dedup cache | Simple, sufficient for single-process lfd; no DB overhead | Persistent dedup in DB — overkill for v1 |
| Derive repo identity from git remote | Avoids storing GitHub identity separately; works with any clone | Store owner/repo on Wave — more coupling |
| Run-level typing (WaveRunKind) | Clean separation without a separate sidecar table; `is_main()` is a single guard | Separate `sidecar_runs` table — more schema complexity |
| Ephemeral worktree per sidecar | Isolates CI fix work from main wave worktree | Reuse main worktree with stashing — conflicts with concurrent main runs |
| Shared debug prompt contract | CI auto-fix and manual `lf debug` use the same prompt | Dedicated CI fix prompt — divergent maintenance |

## How it fits together

```
GitHub webhook → hooks::github_webhook_handler
                     ↓ (verify signature, match to wave)
                 emit Event::CiFailure
                     ↓
              triggers::ci_failure::spawn_ci_failure_handler
                     ↓ (subscribes to EventHub)
              executor::spawn_ci_fix_agent
                     ↓ (acquire scheduler slot, create ephemeral worktree)
              executor::run_ci_fix_agent_with_slot
                     ↓ (create sidecar WaveRun, run debug prompt, push fix)
              cleanup ephemeral worktree
```

The polling path (`poll_all_waves_ci` / `poll_wave_ci`) feeds into the same `emit_ci_failure` function, so dedup and downstream handling are identical regardless of trigger source.

## Risks and bottlenecks

- **Startup polling cost** scales linearly with active PR count (one GitHub API call per wave/branch pair). Fine for small deployments; may need batching or caching for 50+ active PRs.
- **In-memory dedup** resets on daemon restart, so the same failure could trigger a second fix attempt after restart. Acceptable for v1 since the fix agent is idempotent.
- **No retry strategy** — if the CI fix agent fails, the failure is silently logged. A retry or escalation mechanism may be needed.
- **Branch ref assumption** — polling uses `commits/{branch}/check-runs` which assumes GitHub consistently resolves branch names with slashes (URL-encoded).

## What's not included

- Periodic background polling loop (startup + explicit endpoint only).
- Automatic webhook registration with GitHub.
- Structured CI log/annotation ingestion (only metadata: check_name, logs_url, SHA, branch).
- Auto-retry for failed fix attempts.
- Dedicated sidecar scheduling policy (uses normal scheduler slots).
- Rate limiting on the webhook endpoint.
