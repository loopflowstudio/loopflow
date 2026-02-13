# GitHub CI hook + sidecar auto-fix (consolidated)

## Scope and current status

This branch implements GitHub CI failure handling for waves:

- Webhook ingestion at `POST /v0/hooks/github` with HMAC SHA-256 verification (`X-Hub-Signature-256`).
- One-shot CI polling:
  - startup poll when `github.token` / `LFD_GITHUB_TOKEN` is configured,
  - manual poll endpoint `POST /v0/waves/{wave_id}/check-ci`.
- `Event::CiFailure` plus trigger wiring that spawns CI fix sidecars in parallel with main wave runs.
- Sidecar run typing on `WaveRun` (`run_kind`, optional `sidecar_kind`) and DB migration `004_wave_run_kind.sql`.
- Sidecar CI fix flow: ephemeral `.ci-fix.` worktree, shared debug prompt path, push back to PR branch, cleanup.
- Unified startup janitor for stale fork + sidecar ephemeral worktrees.

## Decisions to preserve

- **Single debug path:** CI auto-fix and manual `lf debug` both use the same prompt contract.
- **Run-level typing:** main-vs-sidecar is represented on `WaveRun`; `WaveRun::is_main()` is the primary guard for legacy "active run" behavior.
- **Deduping model (v1):** CI failures dedupe in-memory by `(wave_id, commit_sha)` only for current process lifetime.
- **Repo matching source:** derive `owner/repo` from local git `origin` remote instead of storing separate GitHub identity.
- **Polling behavior:** missing token logs one warning and skips polling (webhooks still work).

## Known limitations and follow-ups

- Polling currently performs one `check-runs` API call per candidate wave/branch, so startup cost scales with active PR count.
- CI context passed to the agent is metadata-level (`check_name`, `logs_url`, SHA, branch); structured annotation/log ingestion is not implemented.
- No automatic webhook registration.
- No periodic background polling loop (startup + explicit endpoint only).
- No auto-retry strategy for unsuccessful fix attempts.
- No dedicated sidecar scheduling policy beyond normal scheduler slot acquisition.

## Active assumptions / questions

- Polling currently calls `GET /repos/{owner}/{repo}/commits/{branch}/check-runs` using a branch ref; this assumes GitHub accepts that ref form consistently.
- CI sidecars create a temporary local branch (`ci-fix-<short-id>`) from `origin/<pr-branch>` and push `HEAD:<pr-branch>` rather than checking out PR branch directly in multiple worktrees.
- Sidecar `WaveRun` snapshots intentionally clear `snapshot.pr` so sidecars do not inflate `open_pr_count` in wave DTOs.
