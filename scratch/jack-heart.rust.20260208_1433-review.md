# Review: jack-heart.rust.20260208_1433

Wave lifecycle overhaul: waves return to idle (not completed) after runs finish, the UI shows git state (commits, diff stat) and offers land/next actions, and the API moves to `/v0`.

## What was implemented

### 1. Remove `WaveStatus::Completed`, waves return to idle

`WaveStatus::Completed` is removed from the enum. After a run finishes, the executor sets the wave to `Idle` — the run is done, but the wave is ready for its next iteration. The Swift client maps any lingering `"completed"` strings to `.idle` during JSON parsing.

### 2. Auto-create draft PR on run completion

When a wave run completes, the executor calls `auto_create_pr` (best-effort) which stages, commits, pushes, and creates a draft PR. The PR reference is stored in the run's snapshot. If any step fails, it logs a warning and continues.

### 3. Store: split `get_active_wave_run` and `get_latest_wave_run`

Previously `get_active_wave_run` included `Failed` status, which was semantically wrong — a failed run is not active. Now:

- `get_active_wave_run` returns only Pending/Running/Waiting runs. Used by triggers to check if a wave is already running.
- `get_latest_wave_run` returns the most recent run regardless of status. Used by the HTTP layer to populate the DTO and by `land_wave_handler`/`next_wave_handler` to find the worktree.

Both SQLite and Postgres implementations added.

### 4. Git state in wave DTO (commits, diff stat)

`build_wave_dto` now calls `infer_wave_git_state` which returns a `WaveGitState` struct with worktree path, branch, commit log, and diff stat. The DTO includes `commits: Vec<CommitEntryDto>` and `diff_stat: Option<String>`. Python models (`CommitEntry`, `Wave.commits`, `Wave.diff_stat`) and Swift models (`CommitEntry`, `Wave.commits`, `Wave.diffStat`) updated to match.

### 5. Next wave endpoint and UI

New `POST /v0/waves/:wave_id/next` endpoint calls `ops::next_branch` to create a new iteration branch. Python client gets `next_wave()`. Swift `LocalWaveService` gets `nextWave()`. `WaveDetailPanel` shows land/next action buttons when commits exist.

### 6. API route prefix: `/v1` → `/v0`

All API routes moved from `/v1` to `/v0`. Updated in Rust router, Python client, and Swift `LoopflowCore`.

### 7. Diff fallback for staged-only branches

`gather_diff_tiered` checks committed changes vs the base branch. If none, falls back to `git diff HEAD` to capture staged/unstaged working tree changes.

### 8. Remove "default" direction sentinel

Empty direction/area arrays now mean "not configured" instead of `["default"]`. Removed filtering in Swift UI and guards against empty arrays in Rust handlers.

### 9. Concerto idle/failed wave UI

Idle waves now show commit log, diff stat, and ops actions (View PR, Land, Next). Failed waves show error detail + StepRunner for retry + live output. The run progress section was simplified — only running waves show `FlowProgressPills`.

## Key choices

**Idle over Completed**: A wave is a long-lived object that iterates. "Completed" implies finality, but waves loop. Idle means "run finished, ready for next." This removes an entire status and the UI logic around it.

**Auto-create PR**: Best-effort, fire-and-forget. If it fails (no remote, auth issue), the wave still succeeds. The PR is stored in the run snapshot, not the wave itself, so each iteration can have its own PR.

**`/v0` not `/v1`**: The API isn't stable yet. `/v0` signals pre-1.0 to clients. Breaking changes are expected.

**Separate `get_latest_wave_run` vs parameterizing**: Clearer intent at call sites — "is something running?" vs "what happened last?" are distinct questions.

## How it fits together

Run completes → executor sets wave to Idle and auto-creates PR → `build_wave_dto` fetches git state (commits, diff stat) and latest run → Concerto shows idle view with commit log, diff stat, and land/next buttons → user clicks Land (merges to main) or Next (creates new iteration branch) → wave is ready for next run.

## Risks and bottlenecks

- **`active_run` DTO field name**: Now refers to the latest run (not necessarily active). The JSON field name is kept for Swift client compatibility, but internal naming could drift.
- **`auto_create_pr` on every completion**: If the wave is configured for looping, each iteration will try to create a PR. Currently best-effort — if a PR already exists, `current_pr` returns it and the snapshot updates. But rapid iterations could create noise.
- **`WaveStatus::Completed` migration**: Existing database rows with status=6 (the old Completed i32 value) will map to `Idle` via `from_i32`'s fallback. The Swift client explicitly maps `"completed"` → `.idle`. No database migration needed but old status values silently change meaning.

## What's not included

- No database migration for old `Completed` status rows — they fall through to `Idle` via the default branch.
- No pagination or truncation for commit log in the DTO — large branches could send many commits.
- The `active_run` DTO field name is unchanged to avoid a Swift client migration.
