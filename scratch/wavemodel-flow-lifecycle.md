# Flow lifecycle: rename, update-wave, loop signal

Three changes to how flows work: rename the core flows for clarity, replace `consolidate` with `update-wave` as the standard post-build step, and use `wave/` existence as the loop/stop signal.

## Phase 1: Rename ship → build, design-ship-review → ship

The current naming is backwards. `ship` (implement → compress → gate → consolidate) is the headless build engine. `design-ship-review` (design → ship → review) is the full interactive lifecycle — the thing you actually "ship" with. Rename:

| Current | New | Steps |
|---------|-----|-------|
| `ship` | `build` | implement → compress → gate → update-wave |
| `design-ship-review` | `ship` | design → build → review |

Cascade through all flows that embed `ship` as a sub-flow:

| Flow | Current | New |
|------|---------|-----|
| `pair` | design → ship | design → build |
| `grind` | research → iterate → ship → gate | research → iterate → build → gate |
| `incident` | debug → 5whys → ship | debug → 5whys → build |
| `ship-wave` | start → ship → update-wave | start → build → update-wave |
| `ship-roadmap` | ingest → kickoff → review-design → ship → review | ingest → kickoff → review-design → build → review |
| `scan` | scan-report → scan-plan → ship | scan-report → scan-plan → build |

Update all references: flow YAMLs, READMEs, Rust default flow (`waves.rs`), Swift `RepoState` default, test fixtures across Rust/Swift/Python.

`design-and-ship` stays as-is (it's design → implement → reduce → polish, doesn't use the `ship` sub-flow).

## Phase 2: update-wave replaces consolidate

`consolidate` reorganizes scratch/ — useful bookkeeping, but not the right terminal step for a build. `update-wave` should be the standard post-build step that:

1. Updates the current wave item's status (done, blocked, needs-more-work)
2. Removes the wave item from `wave/` if the work is complete
3. Moves relevant scratch/ artifacts into the commit or deletes them

`build` becomes: implement → compress → gate → update-wave (replacing consolidate).

The `publish` flow (consolidate → add-to-wave) may need adjustment — `consolidate` still makes sense as a pre-publish step for plan flows. `update-wave` is specifically for the code path where work is being completed, not planned.

## Phase 3: wave/ as loop signal

The wave executor's loop/stop decision becomes simple:

- **Items in `wave/`** → pick next item, continue the flow
- **`wave/` empty or missing** → stop the wave

This replaces any hardcoded loop count or manual stop. A wave runs until its roadmap is done. `update-wave` removing completed items is what naturally drains the queue.

The NUX flow: user creates a wave → `ship` flow starts → design session populates `wave/` with roadmap items → build works through them → update-wave removes completed items → wave stops when `wave/` is empty.

This also means a wave can be restarted by adding new items to `wave/`. No reconfiguration needed — just drop a markdown file and run.
