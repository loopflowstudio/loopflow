# Review: Simplify wave PR merge flow

Branch: `jack-heart.looping.20260212_1227`
Commit: `904c610b` — simplify wave PR merge flow and remove unused UI plumbing

## What was implemented

Three related changes unified under one commit:

1. **Rename collapse/absorb to combine.** The `collapse_prs` operation (and its API, DTOs, Swift UI, tests) was renamed to `combine_prs`. The separate `absorb_into_pr` operation was removed entirely — it was unused UI surface area with no active user flow.

2. **Auto-stimulus PR promotion.** Recurring waves (loop/watch/cron) now auto-promote draft PRs to ready-for-review via `mark_ready`. Manual waves still create drafts. The `is_recurring` check was hoisted above the PR creation call to avoid a redundant `list_stimuli` query.

3. **Operational improvements to lfd:**
   - `lfd install` — new subcommand that generates and loads a macOS LaunchAgent plist, capturing the current `PATH`.
   - `fail_orphaned_runs()` — on startup, marks any Running/Pending/Waiting runs as Failed so stale runs from a crashed lfd don't block the scheduler.
   - `stop` handler now sets wave status to `Paused` (not `Failed`) for waves with auto stimuli, preventing the stimulus loop from immediately restarting a stopped wave.
   - `publish.py` restart uses `lfd install` to regenerate the plist on upgrade.

## Key choices

| Decision | Why |
|----------|-----|
| Remove absorb entirely | No UI flow triggered it; combine handles the multi-PR case. Simpler API surface. |
| Combine = rename, not rewrite | Same cherry-pick logic, just better naming. Added `update_combined_pr_message` to auto-title the combined PR. |
| `mark_ready` promoted to `pub` from `land.rs` | Needed by executor; reuses existing `gh pr ready` wrapper rather than duplicating. |
| Paused vs Failed on stop | Failed status triggers stimulus restart. Paused is a distinct state the scheduler respects. |
| Orphan cleanup on startup | Simpler than trying to resume — orphaned runs have no live process. |

## How it fits together

The wave lifecycle for recurring waves is now: run completes -> auto-create PR (ready, not draft) -> advance branch for next iteration. For manual waves: run completes -> auto-create draft PR. The combine operation is available in the UI when 2+ open PRs exist for a wave.

`lfd install` feeds into the publish flow — after building a new binary, `publish.py` calls `lfd install` to regenerate the plist with the updated binary path and current PATH, then reloads the service.

## Risks and bottlenecks

- **`mark_ready` is best-effort** — if GitHub rate limits or the network is down, the PR stays as a draft. The warn log captures this. Acceptable since the user can manually mark it ready.
- **`fail_orphaned_runs` runs synchronously on startup** for SQLite (the store uses a mutex). For a large number of orphaned runs this is fast (single UPDATE), but the mutex is held briefly during startup.
- **`lfd install` passes `serve` as an argument** to the plist, but the main function's match arm treats any unrecognized command (including `serve`) as a fallthrough to server startup. Works correctly but a future subcommand named `serve` would shadow silently.

## What's not included

- No migration for existing LaunchAgent plists — `lfd install` overwrites the existing one.
- No `lfq` CLI wrapper for combine — it's only exposed via the HTTP API and Concerto UI.
- No backwards compatibility for the `/waves/:id/collapse` or `/waves/:id/absorb` API routes.
