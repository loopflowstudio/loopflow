# Run Visibility — Review Guide

## What was implemented

Live git state during wave runs. When an agent works autonomously, commits and diff stats now appear within 5 seconds instead of waiting for the step to finish.

**Rust:** A `GitStatePoller` background task spawns when a run starts and polls `infer_wave_git_state_for_worktree()` every 5 seconds. When commit SHAs or diff stat change, it emits `WaveUpdated` through the existing event hub. The task is cancelled automatically via `Drop` when the run ends.

**Swift (WaveDetailPanel):** New commits slide in from the top with a brief burgundy highlight (0.3s). Diff stat cross-fades. A small pulsing dot appears next to "Diff" during active runs. Expanded file diffs are invalidated when new commits land. All animations respect `reduceMotion`.

**Swift (RepoState):** Event handler refined — `loadWaveContent` only fires on wave creation and run completion (not every `WaveUpdated` poll). `loadRuns` fires during active runs and on completion. A `didRunComplete` helper detects running/waiting → idle/failed transitions.

**Swift (WaveDetailLiveUpdates):** Pure function `evaluateCommitFeedUpdate` extracts commit-diffing logic for testability. Returns a decision struct: which SHAs are new, whether to animate, whether to invalidate diff cache.

## Key choices

| Decision | Why | Alternative rejected |
|----------|-----|---------------------|
| Poll git state (5s) | Simple, self-contained, ~10ms per poll. No cross-system coupling. | Filesystem watcher (platform-dependent, race conditions), session event bridging (high-frequency, fragile filtering) |
| Reuse `WaveUpdated` event | Zero protocol changes. Enrichment pipeline already builds full DTO. | New `WaveProgress` event (unnecessary protocol expansion) |
| RAII poller lifecycle | `GitStatePollerTask::Drop` aborts the tokio task. No manual cleanup. | Explicit cancel token (more code, same result) |
| Commits as atomic unit | Committed changes are permanent; file edits are speculative. Higher signal-to-noise. | Per-file edit tracking (noisy, couples session internals to wave UI) |
| Separate `evaluateCommitFeedUpdate` | Pure function, trivially testable, no SwiftUI dependencies. | Inline logic in view (harder to test, harder to reason about) |

## How it fits together

```
Wave run starts
  → WaveExecutor::execute() spawns GitStatePollerTask (held on stack)
  → Poller loop: sleep 5s → infer_wave_git_state_for_worktree()
  → Compare SHAs + diff stat to previous snapshot
  → If changed: emit WaveUpdated via EventHub
  → ws.rs enriches WaveUpdated with full WaveDto (already existing)
  → Concerto receives WaveUpdated, updates WaveStore
  → WaveDetailPanel.onChange(of: wave.commits) → applyCommitUpdate()
  → evaluateCommitFeedUpdate() decides: animate? invalidate cache?
  → New commits slide in with burgundy highlight; diff stat cross-fades
Wave run ends
  → execute() returns → GitStatePollerTask dropped → tokio task aborted
  → WaveUpdated with terminal status → handleWaveEvent refreshes content + runs
```

## Risks and bottlenecks

- **Poller fires during rebases/amends.** 5-second interval is slow enough to avoid conflicts — git operations complete well within that window. The poller only reads, never writes.
- **Concurrent waves.** 10 running waves = 10 pollers = ~100ms of git work per 5s interval. Negligible. No throttling needed.
- **Highlight task leak.** `Task.sleep(for: .milliseconds(300))` in `applyCommitUpdate` could outlive the view. SwiftUI cancels Tasks tied to the view lifecycle, so this is safe in practice.
- **`loadRuns` on every poll.** During active runs, `loadRuns` fires every 5 seconds (when git state changes). This is a local HTTP call to lfd (~ms), not a concern.

## What's not included

- Per-file edit tracking during sessions (future: turn-by-turn file changes)
- Session transcript in wave view (that's interactive mode)
- iOS commit feed (macOS only; iOS follows later)
- Revert/checkpoint controls (visibility only, no actions)
- Step indicator work (already live via FlowProgressPills)

## Test coverage

| Layer | Tests | What they cover |
|-------|-------|-----------------|
| Rust | 3 unit tests | Poller ignores initial snapshot, detects commit changes, detects diff stat changes |
| Swift | 4 unit tests | Initial snapshot skips animation, running wave animates, idle wave skips, unchanged commits no invalidation |
| Existing | Wave executor integration test | `execute_emits_wave_updated_on_step_advance` (pre-existing) |

## Verification

```bash
uv run python scripts/concerto-dev.py run-debug   # launch lfd + Concerto
# Start a wave run, observe commits appearing live in the detail panel
# Verify diff stat updates with cross-fade
# Verify pulsing dot on "Diff" header during runs
# Verify animations stop when run completes
# Test with System Settings → Accessibility → Reduce Motion enabled
```
