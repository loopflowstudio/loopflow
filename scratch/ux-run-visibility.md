# Run Visibility

## Problem

When a wave runs autonomously, Concerto goes silent at the structural level. The raw agent output streams (that works), but the wave's *state* — commits, diff stat, runs list, wave content — freezes until the run ends or a step advances. Users who check in see momentum in the output log but no evidence of work landing.

**Who benefits:** Anyone watching a headless wave. Conductor personas especially — they're checking in on multiple parallel workstreams.

**Why now:** RunStore (item 06) landed. The run infrastructure is there. Commits, diff stats, and step tracking are all computed server-side. The gap is purely in *when* that data refreshes.

## Current liveness audit

What updates live during runs today, and what doesn't:

| Signal | Live? | Mechanism |
|--------|-------|-----------|
| Agent raw output | Yes | OutputBuffer → LiveOutput (WebSocket `output_line` events) |
| Flow step progress | Yes | `WaveUpdated` on `advance_run_step()` → FlowProgressPills |
| Elapsed time | Yes | 1s timer in WaveDetailPanel |
| Wave status | Yes | `WaveStarted`/`WaveStopped`/`WaveUpdated` events |
| Step name + index | Yes | Already displayed in FlowProgressPills via `activeRun` |
| **Commits** | **No** | Only refresh on step advance or run end |
| **Diff stat** | **No** | Same — stale for entire step duration |
| **Runs list** | **No** | Loaded on appear only; stale after run completes |
| **Wave content** | **No** | `waveContent` (goals, scratch docs) loaded on appear + status change |
| **File diff cache** | **No** | Expanded file diffs stale after new commits land |

The gap: within a single step, the agent might make 20 commits over 10+ minutes. None appear until the step finishes. Commits are the proof progress is real, and they're invisible.

## Approach

**Make everything live during runs by polling git state and pushing updates through the existing event pipeline. Animate new data arriving so the UI feels alive, not just refreshed.**

No new event types. No session-to-wave coupling. No new API endpoints.

### Rust: Git state poller

When a wave run starts, spawn a background task that polls git state every 5 seconds:

```
run starts
  → spawn GitStatePoller(wave_id, worktree_path)
    → every 5s: infer_wave_git_state()
    → compare commit SHAs to last known
    → if changed: emit WaveUpdated via event_hub
  → run ends: cancel poller
```

`infer_wave_git_state()` already exists and runs in ~10ms (local git operations). The poller only emits `WaveUpdated` when commits or diff stat actually change — no event spam.

The `WaveUpdated` event is already enriched by `ws.rs` with full `WaveDto` including commits, diff stat, and active run. Concerto already handles this event. Zero client-side protocol changes.

### Detail: Poller lifecycle

The poller lives in the wave executor, not the session runtime. This matters:

- Sessions don't know about waves. The poller shouldn't couple them.
- The executor already manages run lifecycle (start/stop). Adding a poller is natural.
- If the agent crashes mid-run, the executor cleans up the poller with the run.

Poller pseudocode:

```rust
struct GitStatePoller {
    interval: Duration,      // 5 seconds
    last_commits: Vec<String>, // SHA list
    last_diff_stat: Option<String>,
}

impl GitStatePoller {
    async fn run(&mut self, wave_id: &str, worktree: &Path, event_hub: &EventHub) {
        loop {
            tokio::time::sleep(self.interval).await;

            if let Some(state) = infer_wave_git_state(repo, wave_name) {
                let commit_shas: Vec<_> = state.commits.iter().map(|c| &c.sha).collect();
                if commit_shas != self.last_commits || state.diff_stat != self.last_diff_stat {
                    event_hub.fire(Event::WaveUpdated { wave_id: wave_id.to_string() });
                    self.last_commits = commit_shas.into_iter().cloned().collect();
                    self.last_diff_stat = state.diff_stat.clone();
                }
            }
        }
    }
}
```

### Swift: Animate the living diff

**Commit feed animation.** Track known commit SHAs as `@State previousCommitSHAs: Set<String>` on WaveDetailPanel. When `wave.commits` changes with new SHAs not in the previous set, animate them in — slide down from the top of the commit list with a brief burgundy highlight (0.3s). Only animate during active runs; on cold refresh (not running), skip animation and just render.

**Live diff stat.** The diff stat summary line ("3 files changed, +42 -18") updates in place with a brief cross-fade. During a run, add a subtle pulse to the diff section header to signal it's live. Pulse stops when the run ends.

**File diff cache invalidation.** When new commits arrive during a run, clear `fileDiffs` for any expanded files so the next expansion fetches the current diff. This prevents stale expanded diffs.

**No mode switch.** The run view IS the wave view. Same sections, same layout. Data flows in live during runs and settles when the run ends.

### Swift: Live auxiliary state

**Runs list refresh.** The `WaveUpdated` event handler in RepoState should call `loadRuns(for: waveId)` when the affected wave has an active run or just transitioned to idle/failed. Currently runs only load on appear — the Runs tab goes stale when a run completes.

**Wave content refresh on run completion.** When a run ends (wave transitions from running to idle/failed), refresh `waveContent` so updated scratch docs and goals are visible without navigating away.

### Detail: Commit animation

```
Wave receives WaveUpdated event
  → Compare wave.commits to previousCommitSHAs (by SHA)
  → New commits = SHAs not in previousCommitSHAs
  → If wave is running and new commits exist:
    → Insert at top of list with slide transition
    → Brief highlight animation (0.3s burgundy flash, then settle)
  → Update previousCommitSHAs
  → If wave is NOT running: just update, no animation
```

Respect `reduceMotion` — when enabled, commits appear instantly without slide or highlight.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Session event bridging | Subscribe to session SSE from Concerto during autonomous runs, surface file edits in real-time | Couples session internals to wave UI. Complex subscription management. Session events are high-frequency (text deltas) — filtering is fragile. Overkill for "which files changed." |
| New `WaveProgress` event type | Dedicated event with file list, step info, commit delta | Unnecessary protocol expansion. `WaveUpdated` already carries everything via enrichment. New event type means new parsing on every client. |
| Filesystem watcher on worktree | Watch `.git/` for ref changes | Platform-dependent. Race conditions with agent's git operations. More moving parts than polling for equivalent latency. |
| Faster polling (1s) | More responsive | Diminishing returns. 5s is fast enough for "checking in." 1s risks conflicts with agent's git operations during rebases or amends. |
| Turn-level bridging via executor | Wave executor subscribes to session turn events, emits file summaries per turn | Tighter coupling than needed. Turns are an agent concept — waves shouldn't care. Git state is the ground truth; file edits are speculative until committed. |

## Key decisions

**Commits are the atomic unit, not file edits.** File edits are speculative — the agent might revert them. Commits are permanent. The live feed shows commits, not individual file touches. This keeps the signal-to-noise ratio high and avoids the "wall of noise" failure mode.

**Poll, don't push.** Polling git state every 5 seconds is simpler and more reliable than instrumenting the session pipeline. Git operations are fast (~10ms). The poller is self-contained — no cross-system coupling. If we later want per-file-edit visibility, that's a separate feature that can layer on top.

**No new protocol.** Reusing `WaveUpdated` means zero changes to the WebSocket protocol, event parsing, or client-side event handling. The enrichment pipeline (`enrich_event` → `build_wave_dto`) already computes everything. We just trigger it more often.

**Animate, don't replace.** New commits slide in; diff stat numbers cross-fade. The UI feels alive without jarring layout shifts.

**Concurrency note.** Each running wave adds one `infer_wave_git_state()` call per 5s (~10ms each). At 10 concurrent waves that's ~100ms of git work per 5s interval — negligible. No throttling needed now.

## Scope

**In scope:**
- Git state poller in wave executor (Rust)
- Poller lifecycle: spawn on run start, cancel on run end
- Commit slide-in animation with burgundy highlight (Swift)
- Live diff stat with cross-fade and pulse indicator (Swift)
- File diff cache invalidation when new commits land (Swift)
- Runs list refresh on `WaveUpdated` events (Swift)
- Wave content refresh on run completion (Swift)
- `reduceMotion` support for all animations (Swift)
- Tests: poller change-detection (Rust), commit SHA diffing and animation triggers (Swift)

**Out of scope:**
- Per-file edit tracking during sessions (future: turn-by-turn file changes)
- Session transcript in wave view (that's interactive mode, not headless visibility)
- Revert/checkpoint controls (visibility only, no actions)
- Commit feed on iOS (macOS first; iOS can follow the same pattern later)

**Clarification:** Step indicator (step name + index) is already live via FlowProgressPills — no work needed.

## Done when

1. Start a wave run (`lf flow build` on a wave with a worktree)
2. Watch the wave detail panel in Concerto while the agent works
3. Within 5 seconds of the agent making a commit, the commit slides in with a burgundy highlight
4. The diff stat summary cross-fades to new values as commits land
5. The diff section header pulses subtly during active runs
6. Expanded file diffs refresh when new commits arrive
7. The Runs tab updates when a run completes (without navigating away)
8. Wave content (goals, scratch docs) refreshes on run completion
9. When the run completes, all animations stop and the final state matches a fresh wave fetch
10. All animations respect `reduceMotion`
11. `cargo test` covers the poller's change-detection logic
12. Swift tests cover commit SHA diffing and animation trigger logic

Validation: `uv run python scripts/concerto-dev.py run-debug`, start a wave, observe commits appearing live.
