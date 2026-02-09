# Review: jack-heart.looping.20260209_1323

## What was implemented
- Recurring wave runs now auto-advance to a new branch after a run that opened a PR, so each loop/watch/cron iteration can get its own PR.
- Wave HTTP DTOs now include `open_pr_count`, and Concerto uses that count (with pending-PR fallback) to group/show stacked PR state in sidebar rows and detail badges.
- Wave actions (`next`, `land`, `collapse`, `absorb`, `continue`) now emit `wave_updated` events so UI state refreshes immediately.
- Wave API handlers now recover from missing/stale stored worktree paths by recreating wave worktrees when needed.
- `lf` Ctrl+C handling now terminates a running child agent before exiting, and PID tracking was hardened to clear on all exit paths.
- `lf ops next` branch naming collision handling now uses shared `branch_exists(...)?`; polish removed the no-op `--block` path from internal options/CLI wiring.

## Key choices
- **Branch-per-iteration for recurring stimuli:** implemented at executor completion time, gated by stimulus kind and PR presence, instead of requiring manual `next`.
- **Open PR count from run snapshots:** computed server-side from wave runs so UI gets one canonical count regardless of client-side derivation.
- **Worktree recovery in API layer:** centralized through `resolve_wave_work_dir_for_api(...)` so wave actions share the same fallback behavior.
- **Event-driven refresh over polling latency:** explicit `wave_updated` emissions were added at state transition points to keep Concerto in sync.
- **PID guard for child processes:** replaced manual set/reset with a drop guard so stale PID state is not left behind on early returns.

## How it fits together
The executor updates run/wave state, creates PR metadata, and now advances branches for recurring runs. HTTP routes surface richer wave DTO data (`open_pr_count`, flow steps, active run) and emit update events after write actions. Concerto consumes these updates, recomputes grouping using `effectiveOpenPRCount`, and presents stacked PR/worktree state in rows, detail headers, and action affordances.

## Risks and bottlenecks
- `open_pr_count` is derived from persisted run snapshot PR states; if external PR state drifts and snapshots are stale, counts can lag reality.
- Branch auto-advance depends on git/remote push success; failures are logged but non-fatal, so later iterations may still require manual cleanup.
- `build_wave_dto` now does an additional `list_wave_runs` query per wave; large wave histories can increase list latency.
- Nearest-base diff inference depends on local remote refs/merge-base availability; unusual ref states can still degrade commit/diff context.

## What's not included
- No migration of persisted historical run data to normalize PR states.
- No UI redesign beyond count badges, grouping, and success/toast affordances.
- No changes to external/public API versioning for new DTO fields.
- No deeper optimization of wave list aggregation queries beyond this pass.

## Validation
- `cargo fmt --all`
- `cargo clippy --all -- -D warnings`
- `cargo test --all`
- `uv run pytest python/tests/`
- `swift test --package-path swift`
- `tests/e2e/test_smoke.sh`
- `cd swift && xcodegen generate && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS'`
