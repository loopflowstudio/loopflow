# Flow lifecycle: rename, update-wave, loop signal

## Problem

Three naming/behavior issues compound into a confusing wave lifecycle:

1. **`ship` names the wrong thing.** `ship` (implement → compress → gate → consolidate) is the headless build engine. `design-ship-review` (design → ship → review) is what you actually "ship" with. The names are backwards.

2. **`consolidate` is the wrong terminal step.** A build should end by recording what was done against the wave plan, not reorganizing scratch/. `update-wave` already exists for this purpose but isn't wired into `build`.

3. **Waves don't know when they're done.** The loop ticker fires every 5s regardless of whether work remains. A wave with `Loop` stimulus runs forever until manually stopped. The wave/ directory — the source of truth for remaining work — is never consulted.

These three changes are tightly coupled: the rename enables the `update-wave` swap, and `update-wave` draining wave/ items enables the loop signal.

## Approach

Three phases, each building on the last. All three ship in one PR.

### Phase 1: Rename ship → build, design-ship-review → ship

Mechanical rename across the codebase.

| Current | New | Steps |
|---------|-----|-------|
| `ship` | `build` | implement → compress → gate → update-wave |
| `design-ship-review` | `ship` | design → build → review |

Cascade through all flows that embed `ship` as a sub-flow:

| Flow | Current steps | New steps |
|------|---------------|-----------|
| `pair` | design → ship | design → build |
| `grind` | research → iterate → ship → gate | research → iterate → build → gate |
| `incident` | debug → 5whys → ship | debug → 5whys → build |
| `ship-wave` | start → ship → update-wave | start → build → update-wave |
| `ship-roadmap` | ingest → kickoff → review-design → ship → review | ingest → kickoff → review-design → build → review |
| `scan` | scan-report → scan-plan → ship | scan-report → scan-plan → build |

`design-and-ship` stays as-is — it uses design → implement → reduce → polish, no sub-flow reference.

**Files to change:**

Flow YAMLs (rename + update references):
- `rust/loopflow/src/engine/builtins/flows/code/ship.yaml` → rename to `build.yaml`
- `rust/loopflow/src/engine/builtins/flows/code/design-ship-review.yaml` → rename to `ship.yaml`
- `rust/loopflow/src/engine/builtins/flows/code/pair.yaml` — `ship` → `build`
- `rust/loopflow/src/engine/builtins/flows/code/grind.yaml` — `ship` → `build`
- `rust/loopflow/src/engine/builtins/flows/code/incident.yaml` — `ship` → `build`
- `rust/loopflow/src/engine/builtins/flows/code/ship-wave.yaml` — `ship` → `build`
- `rust/loopflow/src/engine/builtins/flows/code/ship-roadmap.yaml` — `ship` → `build`
- `rust/loopflow/src/engine/builtins/flows/scan/scan.yaml` — `ship` → `build`

Rust defaults and discovery:
- `rust/loopflow/src/lfd/http/routes/waves.rs` line 183 — default flow `"ship"` → `"build"`
- `rust/loopflow/src/lf/discovery.rs` — flow descriptions referencing `ship`

Swift defaults:
- `swift/LoopflowCore/State/RepoState.swift` — `createWave` default `"ship-roadmap"` stays, `createAndRunWave` default `"design-ship-review"` → `"ship"`
- `swift/LoopflowCore/Services/LocalWaveService.swift` — default `"ship-roadmap"` stays
- `swift/Concerto/Platform/macOS/Views/FlowProgressPills.swift` — preview steps

Wave configs on disk:
- `wave/living/living.yaml` — `flow: ship-wave` stays (unchanged)
- `wave/mobile/mobile.yaml` — `flow: ship-wave` stays (unchanged)

Test fixtures:
- `rust/loopflow/src/lfd/http/routes/mod.rs` — `flow: "ship"` → `"build"`
- `rust/loopflow/src/lfd/http/routes/hooks.rs` — `flow: "ship"` → `"build"`
- `rust/loopflow/src/lfd/http/routes/wave_config.rs` — `flow: ship` → `build`
- `rust/loopflow/src/lfd/queue.rs` — `flow: "ship"` → `"build"`
- `swift/ConcertoTests/WaveTests.swift` — `flow: "ship"` → `"build"`, hardcoded `flowSteps` arrays
- `swift/ConcertoTests/WaveStoreTests.swift` — `makeWave(flow: "ship")` → `"build"`
- `swift/ConcertoTests/WaveRowTests.swift` — `makeWave(flow: "ship")` → `"build"`
- `swift/ConcertoTests/PortfolioRepoStateTests.swift` — `flow: "ship"` → `"build"`
- `swift/ConcertoTests/RunStoreTests.swift` — `flow: "ship"` → `"build"`
- `swift/ConcertoTests/RepoStateInteractiveSessionTests.swift` — `"design-ship-review"` → `"ship"`
- `python/tests/conftest.py` — `flow_steps` containing `"ship"` → `"build"`
- `python/tests/test_models.py` — assertion on `flow_steps`

Documentation:
- `README.md` — flow tables, examples

### Phase 2: update-wave replaces consolidate in build

Swap the terminal step of `build` from `consolidate` to `update-wave`.

`build.yaml` becomes:
```yaml
- implement
- compress
- gate
- update-wave
```

`consolidate` stays in the `publish` flow (consolidate → add-to-wave) — it's the right pre-publish step for plan flows where you're organizing proposals, not completing work.

The `ship-wave` flow currently has `start → ship → update-wave`. After this change, `build` already ends with `update-wave`, so `ship-wave` would run `update-wave` twice. Fix: `ship-wave` becomes `start → build` (dropping the explicit `update-wave` since `build` now includes it).

Updated `ship-wave.yaml`:
```yaml
- start
- build
```

### Phase 3: wave/ as loop signal

Add a wave/ directory check to the loop ticker. When wave/ for a given wave has no remaining items (no `*.md` files excluding `README.md` and `*.yaml`), skip the run.

**Why this works:** `advance_branch` creates new branches from the worktree HEAD (`git checkout -b`), not from `origin/main`. So when `update-wave` removes a completed item from `wave/<name>/` and commits, that removal carries forward to the next run's branch. The wave/ directory on the worktree is the authoritative state.

**Implementation in `loop_ticker.rs`:**

After the existing checks (not paused, no active session, no active run), add:

```rust
// Check if wave/ has remaining items
let worktree = match store.get_wave_worktree(&wave).await {
    Ok(Some(wt)) => wt,
    _ => continue,
};
if wave_backlog_empty(&worktree, wave.name()) {
    tracing::info!(wave = %wave.name(), "wave/ empty, skipping loop tick");
    // Optionally: auto-pause the wave
    continue;
}
```

`wave_backlog_empty` checks `wave/<name>/` in the worktree for `.md` files that aren't `README.md`. If none exist, the backlog is drained.

**Edge case — first run:** Before any run, the worktree may not exist yet (it's created by `create_wave_run_with_id`). If there's no worktree, fall through to the existing behavior (start the run; the `ingest`/`start` step will populate wave/ or the agent will find nothing to do).

**Edge case — wave/ populated by design step:** The first run through `ship` (design → build → review) may populate wave/ during the design step. The loop signal only matters for subsequent runs. Since the loop ticker checks *before* spawning a run, this works naturally.

**Behavior change:** A wave with `Loop` stimulus now auto-stops when its backlog is drained. No manual `stop_wave` needed. To restart, add new items to wave/ and re-run.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Keep `ship` name, rename `design-ship-review` to `deliver` | Less churn | "Build" is the universal word for headless compilation. "Ship" means getting it to users. The current names fight intuition. |
| Database-tracked wave items instead of filesystem | Daemon has full visibility | Agents already work with wave/ on disk. Adding DB tracking duplicates state. The filesystem is the source of truth — read it directly. |
| update-wave writes a signal file for the loop ticker | Explicit contract between step and daemon | Unnecessary indirection. The daemon can read wave/ directly. Signal files add a protocol to maintain. |
| Check wave/ in executor at run start (early exit) | Catches edge cases | The loop ticker is the right place — don't create a run just to immediately abort it. But could be added as defense-in-depth later. |
| Keep consolidate in build, add update-wave after | Preserves scratch/ cleanup | Build should be about the work, not housekeeping. Scratch/ cleanup can happen in the PR land step or not at all. |

## Key decisions

**`build` is the new default flow for API wave creation.** The `waves.rs` default changes from `"ship"` to `"build"`. This is the right default — most programmatic wave creation wants headless builds, not interactive design sessions.

**`ship-wave` drops its explicit `update-wave`.** Since `build` now includes `update-wave`, the old `start → ship → update-wave` would double-update. New: `start → build`.

**The loop ticker reads the filesystem.** This is a new responsibility for the daemon — it currently only reads wave/ at creation time. The cost is one directory listing per tick per looping wave (cheap). The benefit is waves that self-terminate.

**No auto-pause on empty.** When wave/ is empty, the loop ticker skips the run but doesn't change wave status to Paused. The wave stays Idle with its Loop stimulus active. If someone adds items to wave/, the next tick picks them up. This is simpler and more useful than requiring an explicit re-run.

## Scope

**In scope:**
- Rename flow YAMLs and all references across Rust, Swift, Python, tests
- Swap consolidate → update-wave in the build flow
- Collapse ship-wave from 3 steps to 2
- Add wave/ backlog check in loop_ticker.rs
- Update README flow tables
- Update RELEASE_NOTES if needed

**Out of scope:**
- Changing the `update-wave` step prompt (it already does the right thing)
- Changing the `consolidate` step (it stays in `publish`)
- Changing wave config format or YAML schema
- Adding wave item status to the daemon database
- Changing `advance_branch` behavior
- Renaming `design-and-ship` (it doesn't use the `ship` sub-flow)

## Done when

```bash
# All tests pass with new names
cargo test --all
uv run pytest python/tests/
swift test --package-path swift

# No remaining references to old names in flow definitions
rg '"ship"' rust/loopflow/src/engine/builtins/flows/  # should only match scan desc or similar
rg 'design-ship-review' rust/ swift/ python/          # should return nothing

# Loop ticker has wave/ check
rg 'wave_backlog_empty\|backlog.*empty' rust/loopflow/src/lfd/triggers/loop_ticker.rs

# build flow ends with update-wave
cat rust/loopflow/src/engine/builtins/flows/code/build.yaml
# → implement, compress, gate, update-wave

# ship-wave is 2 steps
cat rust/loopflow/src/engine/builtins/flows/code/ship-wave.yaml
# → start, build
```
