# Flow Lifecycle: Rename, Unify, Simplify

## What was implemented

Renamed the core flow hierarchy for clarity and consolidated post-work reconciliation into a single step:

- **`ship` → `build`**: The headless implementation flow (`implement → compress → gate → update-wave`).
- **`design-ship-review` → `ship`**: The interactive flow (`design → build → review`).
- **`consolidate` + `add-to-wave` + `publish` → removed**: All post-work reconciliation now lives in `update-wave`.
- **Loop ticker**: Simplified from backlog-scanning heuristic to directory-presence check. Wave dir present = work remains; wave dir removed = done.

49 files changed across Rust, Swift, Python, YAML, and docs. All dependent flows (`pair`, `grind`, `incident`, `ship-wave`, `ship-roadmap`, `scan`) updated to route through `build`. Plan flows (`wave-reduce`, `wave-polish`, `wave-expand`) end in `update-wave` directly.

## Key choices

**Directory presence over backlog parsing.** The loop ticker originally scanned for actionable `.md` files (excluding `README.md`) inside `wave/<name>/`. Simplified to: if the directory exists, the wave has work. No content heuristics. Simpler, less fragile, convention-enforced.

**`update-wave` absorbs three steps.** Rather than keeping `consolidate` (reorganize scratch/) and `add-to-wave` (promote to wave/) as separate steps, `update-wave` now owns the full reconciliation: update roadmap status, promote actionable scratch items, merge/dedupe collisions, clean up promoted files. Fewer steps means fewer handoff points and less surface area for token waste.

**`build` as the headless primitive.** Every flow that does headless work routes through `build` as a sub-flow. This makes `build` the single entry point for headless execution, and `ship` the interactive wrapper around it.

## How it fits together

`build` is the engine: `implement → compress → gate → update-wave`. All headless flows compose it. `ship` wraps it interactively: `design → build → review`. The loop ticker uses directory presence to decide whether to start a new run. `update-wave` is the only post-work step — it updates wave state and promotes scratch artifacts.

## Risks and bottlenecks

- **Breaking rename.** External scripts using the old `ship` (headless) name, `design-ship-review`, `publish`, `consolidate`, or `add-to-wave` will break. Migration is mechanical (rename references) but unannounced.
- **Directory presence is coarse.** The ticker can't distinguish "wave has 10 items" from "wave has only a README." Convention must enforce that only actionable items live in the wave dir. Accepted trade-off — simpler is better than a status-field parser.
- **Stale references in other waves.** `wave/living/` still references `consolidate` in its design docs. Out of scope for this PR but noted for follow-up.
- **Docker tests fail without socket.** 2 pre-existing Docker test failures on machines without `/var/run/docker.sock`. Not caused by this branch.

## What's not included

- Per-run/sidecar worktree architecture
- Wave config schema changes
- DB-backed wave item tracking
- Auto-pause behavior on empty backlog
- Renaming `design-and-ship` (it doesn't use the `ship` sub-flow)
- Changes to `advance_branch`
