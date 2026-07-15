# Stable Wave surface

## Problem

The selected Wave is assembled from several partial readings. `lf status` already
returns one complete `WaveDetailSnapshot`, but `RegistryQuery.status` reshapes it
into `WaveStatusResult` and drops the Wave snapshot and Home runtime. The plan view
then keeps only the work map and clears that live Project/Task hierarchy whenever
one refresh fails. A brief local-store or subprocess failure therefore makes a
known Wave look structurally different instead of showing the last trustworthy
reading with an explicit warning.

The Mac surface needs one stable read contract before adding more Wave conduct
interactions. This first PR makes the existing status snapshot that contract.

## The demo

Open a registered Wave in the Mac app, let its Project/Task hierarchy load, then
make a later `lf status` refresh fail. The hierarchy remains visible and the pane
shows `Wave status unavailable: …`; a successful refresh replaces the stale reading
and clears the warning.

## Approach

- Mirror Rust `WaveDetailSnapshot` as a public Swift `WaveDetailSnapshot`, with
  every wire field retained and no defaults.
- Return that snapshot directly from `RegistryQuery.status` instead of creating
  the lossy `WaveStatusResult` projection.
- Expose `workMap` as a computed presentation projection on the snapshot, so the
  selected Wave view can keep its current rendering without a second stored
  shape.
- Keep the last successful detail snapshot when polling fails. Preserve the
  authored plan as the initial/offline fallback and render the refresh error
  alongside whichever trustworthy state is available.
- Drive initial load, periodic polling, and child-activity refreshes through one
  cancellation-aware task, so an older overlapping query cannot replace a newer
  reading.

## De-risking

| Question | Finding | Impact on design |
|----------|---------|------------------|
| Is a new Rust endpoint or DTO required? | No. `lf status --json` already emits Wave identity, purpose, Projects, Tasks, runs, attention, and Home runtime in `WaveDetailSnapshot`. | Keep this PR in Swift and use the existing wire contract. |
| Does keeping stale data hide failure? | The view already has an explicit error banner. The current bug is that it clears live data as well as reporting the error. | Retain last-good data and keep the warning visible until recovery. |
| Should the reverted trajectory UI be restored as the stable surface? | No. #925 explicitly reverted that interaction because it was not the desired conduct model. | Do not add memory reads, writes, or trajectory UI. |
| Can an authored but unregistered Wave use this snapshot? | No registry row means `lf status` has no detail snapshot. Its authored `GOAL.md` plan is already the correct fallback. | Preserve the existing unregistered path without a synthetic wire DTO. |

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Add trajectory and evidence back to the plan pane | Immediately exposes more Wave state. | It repeats a direction deliberately reverted in #925 and couples the shell to an unsettled interaction. |
| Introduce an observable `WaveSurfaceStore` | Centralizes polling and state transitions. | One view owns this reading today; a new store would be abstraction without a second consumer. |
| Leave `WaveStatusResult` and add its missing fields | Smaller rename diff. | It preserves a second DTO-like shape whose only job is to discard fields from the canonical snapshot. |

## Key decisions

The Rust wire name is the domain name in Swift too. `WaveDetailSnapshot` remains
the one truthful status reading; `WaveWorkMap` is only a computed view of it.

Refresh failure does not erase evidence. A last-good snapshot is stale but known;
the warning says the refresh failed. Clearing it turns known state into an
invented empty or static state.

## Scope

- In scope: Swift status DTO/API alignment, last-good Project/Task rendering,
  fixture/query tests, and nearby documentation.
- Out of scope: trajectory/memory UI, Home controls in Wave detail, runs and
  attention rendering, new Rust wire fields, polling cadence, and iOS UI.

## Done when

```bash
swift test --package-path swift
```

The status query test proves Wave and Home fields survive decoding, the shared
fixture decodes as `WaveDetailSnapshot`, and the Mac view retains its last
successful detail snapshot when a later refresh errors.
