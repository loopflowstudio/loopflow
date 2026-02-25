# Flow lifecycle: rename, unify update-wave, loop signal

## Problem

Four naming/behavior issues compound into a confusing wave lifecycle:

1. **`ship` names the wrong thing.** `ship` (implement → compress → gate → consolidate) is the headless build engine. `design-ship-review` (design → ship → review) is what you actually "ship" with. The names are backwards.

2. **Post-run wave maintenance is fragmented.** `consolidate`, `add-to-wave`, and `update-wave` overlap. The split forces users and flows to guess which post-work prompt to run.

3. **Build completion is miswired.** A build should end by reconciling wave state, not scratch housekeeping. `update-wave` should be the terminal build step.

4. **Loop waves don't know when they're done.** The loop ticker fires every 5s regardless of remaining backlog. `wave/` (the source of truth for queued work) is never consulted between runs.

These changes are coupled: renaming clarifies intent, one canonical `update-wave` simplifies lifecycle behavior, and loop-ticker backlog checks let waves self-drain.

## Approach

Three phases, shipped in one PR.

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
| `ship-wave` | start → ship → update-wave | start → build |
| `ship-roadmap` | ingest → kickoff → review-design → ship → review | ingest → kickoff → review-design → build → review |
| `scan` | scan-report → scan-plan → ship | scan-report → scan-plan → build |

`design-and-ship` stays as-is — it uses design → implement → reduce → polish and does not reference the `ship` sub-flow.

### Phase 2: One canonical post-work prompt — `update-wave`

Unify post-work behavior under `update-wave`.

#### Flow changes

`build.yaml` becomes:
```yaml
- implement
- compress
- gate
- update-wave
```

Plan flows stop routing through `publish` and call `update-wave` directly:

- `wave-reduce`: `fork(reduce×3) → update-wave`
- `wave-polish`: `fork(polish×3) → update-wave`
- `wave-expand`: `fork(expand×3) → update-wave`

`ship-wave` becomes:
```yaml
- start
- build
```

#### Deletions

- Delete step: `consolidate`
- Delete step: `add-to-wave`
- Delete flow: `publish`

#### New `update-wave` contract

`update-wave` now owns all post-work reconciliation:

1. Update roadmap/status in `wave/<wave>/`
2. Promote unfinished/actionable items from `scratch/` into `wave/<wave>/`
3. Merge/dedupe collisions in `wave/<wave>/` (no silent overwrite)
4. Remove promoted scratch artifacts

This removes ambiguity: there is one post-work step, one behavior model.

### Phase 3: `wave/` backlog as loop signal (between runs only)

Add a backlog check in `loop_ticker.rs` after existing guards (not paused, no active session, no active run), before creating a run.

#### Canonical workspace model (locked for this PR)

**One canonical worktree per wave.** Use the wave worktree path derived from repo + wave name.

- No sidecar/per-run worktree changes in this PR
- No concurrent runs sharing the same wave worktree
- Backlog is checked only at run boundaries (ticker), never mid-run

#### Implementation sketch

```rust
let worktree = worktree_path(Path::new(wave.repo()), wave.name());
if worktree.exists() && wave_backlog_empty(&worktree, wave.name()) {
    tracing::info!(wave = %wave.name(), "wave backlog empty, skipping loop tick");
    continue;
}
```

`wave_backlog_empty` checks `wave/<name>/` for actionable markdown items:

- count `*.md`
- exclude `README.md`
- ignore `*.yaml`

If no actionable markdown files remain, skip starting a new run.

#### Semantics

Backlog-empty means **no queued wave items**. It does **not** guarantee the most recent run succeeded.

(With current lifecycle, `ingest` moves a picked item from `wave/` to `scratch/` early; failed work may therefore live in `scratch/` until a later `update-wave` reconciliation.)

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Keep `consolidate` + `add-to-wave` + `update-wave` | Smaller prompt changes | Keeps lifecycle ambiguous; users must pick between overlapping prompts |
| Keep `publish` flow and swap internals | Less flow churn | Adds indirection with no user value once `update-wave` is canonical |
| Per-run worktrees now | Cleaner long-term isolation | Requires defining canonical backlog state first; too large for this PR |
| Check backlog inside executor after run creation | Defensive | Wastes run creation overhead; ticker is the right decision point |

## Key decisions

**`build` is the new default flow for API wave creation.** `waves.rs` default changes from `"ship"` to `"build"`.

**`update-wave` is the only post-work step.** Remove `consolidate`, `add-to-wave`, and `publish`.

**Loop ticker reads backlog from the canonical wave worktree.** This PR locks to one worktree per wave.

**No auto-pause on empty.** Empty backlog skips new loop runs but wave status remains Idle with Loop stimulus enabled.

## Scope

**In scope:**
- Rename flow YAMLs and all references across Rust, Swift, Python, tests
- Rename `ship.yaml` → `build.yaml`, `design-ship-review.yaml` → `ship.yaml`
- Replace build terminal step with `update-wave`
- Collapse `ship-wave` to `start → build`
- Delete `consolidate` step
- Delete `add-to-wave` step
- Delete `publish` flow
- Point plan flows to `update-wave`
- Update `update-wave` prompt to include scratch→wave promotion and cleanup
- Add loop ticker backlog check against canonical wave worktree
- Update README and builtins flow docs

**Out of scope:**
- Per-run worktree architecture changes
- Sidecar/worktree isolation changes
- Wave config schema changes
- DB-backed wave item tracking
- Changing `advance_branch` behavior
- Renaming `design-and-ship`

## Done when

```bash
# Tests
cargo test --all
uv run pytest python/tests/
swift test --package-path swift

# Rename complete
rg 'design-ship-review' rust/ swift/ python/              # no matches
rg '\bflow:\s*ship\b|"ship"' rust/loopflow/src/lfd      # only intentional usages

# Consolidation removed
rg '\bconsolidate\b|\badd-to-wave\b' rust/loopflow/src README.md swift/ python/  # no step/flow refs
rg '\bpublish\b' rust/loopflow/src/engine/builtins/flows  # no publish flow refs

# Flow definitions
cat rust/loopflow/src/engine/builtins/flows/code/build.yaml
# -> implement, compress, gate, update-wave

cat rust/loopflow/src/engine/builtins/flows/code/ship-wave.yaml
# -> start, build

cat rust/loopflow/src/engine/builtins/flows/plan/wave-reduce.yaml
cat rust/loopflow/src/engine/builtins/flows/plan/wave-polish.yaml
cat rust/loopflow/src/engine/builtins/flows/plan/wave-expand.yaml
# -> each ends with update-wave

# Loop ticker check exists and uses canonical worktree path
rg 'wave_backlog_empty|worktree_path\(' rust/loopflow/src/lfd/triggers/loop_ticker.rs
```
