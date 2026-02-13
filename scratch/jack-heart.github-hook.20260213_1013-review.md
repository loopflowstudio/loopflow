# GitHub CI hook + sidecar auto-fix review

## What was implemented

- Added GitHub CI failure ingestion in `lfd` via `POST /v0/hooks/github` with HMAC SHA-256 signature verification (`X-Hub-Signature-256`).
- Added one-shot CI polling support:
  - Startup poll for all waves when `github.token` / `LFD_GITHUB_TOKEN` is set.
  - Manual poll endpoint `POST /v0/waves/{wave_id}/check-ci`.
- Added `Event::CiFailure` and a CI failure trigger that spawns sidecar fix agents in parallel with main wave runs.
- Added sidecar run modeling (`WaveRunKind`, `SidecarKind`) and DB migration `004_wave_run_kind.sql` with main-run filtering for active/latest run queries.
- Added sidecar CI fix execution flow:
  - ephemeral `.ci-fix.` worktree creation from PR branch,
  - debug prompt reuse with CI metadata context,
  - commit/push back to PR branch,
  - cleanup of ephemeral worktree.
- Added unified startup worktree janitor for stale fork and sidecar worktrees.

## Key choices

- **Prompt reuse over specialization:** CI auto-fix uses the same debug prompt path as manual `lf debug`.
- **Run-level typing:** Main vs sidecar is encoded on `WaveRun` (`run_kind`) with optional `sidecar_kind`, plus `WaveRun::is_main()` helper.
- **In-memory dedupe:** CI failures are deduped per `(wave_id, commit_sha)` for process lifetime; no DB persistence in v1.
- **Repo identity by origin remote:** GitHub repo matching is derived dynamically from local `origin` URL.
- **Fail-soft polling:** Missing token skips startup poll with a single warning.

## How it fits together

GitHub webhook/poll paths produce `CiFailure` events. The scheduler now runs a CI failure trigger that listens for those events and launches sidecar runs through `WaveExecutor::spawn_ci_fix_agent`. Sidecars run the debug step in an ephemeral worktree, push fixes to the PR branch, and are excluded from main-run concurrency and active-run queries via `run_kind` filtering. A startup janitor reconciles DB-active ephemeral worktrees against git worktree state and removes stale leftovers.

## Risks and bottlenecks

- GitHub polling currently does one HTTP call per target wave/branch (`commits/{branch}/check-runs`), so startup latency scales with active PR count.
- CI log URL is passed through, but full structured annotation ingestion is deferred; fix quality depends on agent reproduction.
- Deduping by process-local cache means daemon restart clears dedupe history.
- Sidecar run update failures can still leave inconsistent run metadata, though cleanup is now attempted even when update persistence fails.

## What's not included

- No automatic GitHub webhook registration.
- No periodic background CI poller (startup + explicit endpoint only).
- No automatic retry loop for failed fix attempts.
- No structured check-run annotation/log download pipeline.
- No dedicated sidecar scheduler prioritization beyond normal slot acquisition.
