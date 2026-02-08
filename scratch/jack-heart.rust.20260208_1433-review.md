# Review: jack-heart.rust.20260208_1433

Four changes shipping together: diff fallback for staged-only branches, store split for active vs latest runs, default direction removal, and failed wave UI.

## What was implemented

### 1. Diff fallback for staged-only branches (`prompt.rs`)

`gather_diff_tiered` now checks whether the branch has committed changes vs the base branch. If not (e.g. a branch with only staged but uncommitted changes), it falls back to `git diff HEAD` to show the working tree diff instead of returning nothing.

Extracted `gather_diff_tiered_with_ref` to avoid duplicating the file-counting and tiered diff logic between the two code paths.

### 2. Store: split `get_active_wave_run` and `get_latest_wave_run`

Previously `get_active_wave_run` included `Failed` status, which was semantically wrong — a failed run is not active. Now:

- `get_active_wave_run` returns only Pending/Running/Waiting runs. Used by triggers (loop ticker, watch, cron) to check if a wave is already running.
- `get_latest_wave_run` returns the most recent run regardless of status. Used by the HTTP layer to populate the DTO (so the UI can show error details for failed runs) and by `land_wave_handler` (to find the worktree path).

Both SQLite and Postgres implementations added.

### 3. Remove "default" direction sentinel

- `LocalWaveService.createWave` now sends `direction: []` instead of `["default"]`
- `DirectionTypeahead.onAppear` no longer filters out `"default"` from selected directions
- `WaveDetailPanel` no longer hides direction display when it equals `"default"`
- `update_wave_handler` and `run_wave_handler` no longer guard against empty direction/area arrays — empty is now a valid state meaning "no direction configured"

### 4. Failed wave detail in Concerto

`WaveDetailPanel.blendedView` now has a dedicated branch for `wave.status == .failed` that shows `failedRunDetail` (error message) + `StepRunner` (to retry) + live output if available. Previously failed waves fell through to the running/config view.

## Key choices

**Fallback to `HEAD` not `--staged`**: Using `git diff HEAD` catches both staged and unstaged changes. `--staged` alone would miss unstaged modifications. The intent is "show what's different from the last commit" when there are no commits ahead of the base.

**Separate trait methods vs status parameter**: Adding `get_latest_wave_run` as a separate method rather than parameterizing `get_active_wave_run` with a status filter. Clearer intent at call sites — you either want "is something running?" or "what happened last?".

**Empty direction/area as valid**: Rather than a sentinel value like `["default"]`, empty arrays mean "not configured." Simpler model, no special-case filtering throughout the UI.

## How it fits together

The store split enables the UI change: `build_wave_dto` calls `get_latest_wave_run` so the `active_run` DTO field always includes the most recent run (even if failed), while triggers still use `get_active_wave_run` to avoid re-triggering waves that failed. The Concerto UI then renders failed waves with error details because the DTO now contains the failed run data.

## Risks and bottlenecks

- **`build_wave_dto` naming**: The function parameter `include_active_run` and DTO field `active_run` now refer to the "latest" run, not strictly an "active" one. This is an intentional API stability choice — the JSON field name is a contract with the Swift client — but internal naming could drift in clarity over time.
- **No remote fallback edge case**: If `origin/<base>` doesn't exist (e.g. fresh local repo with no remote), the `git diff` command fails and the code falls back to `HEAD`. This is correct behavior but worth noting.

## What's not included

- No migration of existing waves that have `["default"]` direction stored in the database. They'll display "default" in the direction field until updated. This is acceptable — directions are user-facing labels, not keys.
- The `active_run` DTO field name is unchanged to avoid a Swift client migration.
