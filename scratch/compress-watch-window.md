# Watch retention compression — 2026-09-24

No further coherent model/API reduction found in HEAD `7b9d5d87d` plus the
working retention slice. This pass adds only this report; implementation,
tests, design edits and evidence remain intact.

## Model before and after

Unchanged: FlowPosition authorizes execution. Transactional Task events retain
plans, attempts, transitions and settlement. Rust projects those facts and Run
manifests into TaskWatchSnapshot; TaskOutputPage separately carries attributed
output and source evidence. RegistryQuery transports both. Navigation retains
TaskWatchStore, while mounted views own cancellable reads. TaskWatchOutput keeps
one chosen revision per loaded source record; rows and contiguous groups are
derived. The store trims records across Runs and retains independent history
and live continuations.

No command, DTO, field, type, persistence path or API was removed by this pass.
The working implementation already clears the latest page's records after
merging, eliminating its duplicate retained payload.

## Path and mirrors inspected

Read `work/task/flow_history.rs`, the transactional history reader and position
receipt writer in `store/sqlite/durable.rs`, and the Watch/output read envelopes.
Compared Rust and Swift Watch/output fields, including stage coordinates,
attempt state, source evidence and required collections. Inspected the shared
output fixture, both fixture envelopes and Rust/Swift fixture assertions.

Followed shared read-only Task lookup through Watch/Output operations and
RegistryQuery's typed reads and temporary cursor transport. Traced navigation's
per-Task store through SessionsView into TaskWatchView, output merging, folding,
retention and recovery controls. Read the budget/restart regressions, accounting
for invisible events, active design and README changes. Searches found none of
TaskOutputRecord, TaskWatchStepKind, containsRevision or latestOutputSource in
the inspected Watch paths.

## Candidates retained

- **Display position, live precedence, Follow and retention recency:** a changed
  historical revision needs eviction priority without moving its row or becoming
  a live arrival. A historical copy must not overwrite a known live revision.
  These facts cannot derive from one shared timestamp or position.
- **Cached payload size:** deriving size during every trim would repeatedly
  serialize retained tool inputs and invisible evidence. The cache changes with
  the chosen revision and stays inside its record owner; it introduces no second
  payload or wire field.
- **Page metadata and accumulated records:** clearing `source.records` leaves
  one retained payload owner while preserving the shared source contract. A new
  metadata-only mirror would add another hand-maintained shape. Source health,
  reset and gaps cannot be inferred from the loaded record map.
- **Omission notice and missing-call labels:** once eviction occurs, a later
  smaller window cannot establish that its history is complete. The notice must
  survive until recovery. Tool correlation derives only from retained records;
  keeping evicted calls elsewhere would undo the bound.
- **Restart history and Reload output:** restart accepts a successful replacement
  history page while preserving live continuation and inspection. Reload resets
  both readers after source replacement. Combining them loses recovery behavior.
  Replacing their booleans with an enum alone would relocate the distinction,
  without removing an owner or public concept.
- **Snapshot, source inventory and transcript window:** auxiliary Runs can have
  no stage, and recorded attempts can lack readable manifests. Neither inventory
  derives from the retained output. Bounding inventories and navigation retention
  requires an explicit discovery/recovery design, not deleting their evidence.

## Verification and remaining work

No tests or builds rerun because executable content is unchanged. Inspected the
existing [15-test receipt](watch-window-evidence/tests.log) and subsequent
[one-test render receipt](watch-window-evidence/render.log). All nine source/test
hashes still match [the recorded manifest](watch-window-evidence/source-hashes.json).
These are prior implementation receipts, not fresh configured interaction proof.
Whitespace checked with `git diff --check`.

The per-Task record/payload window does not bound incoming decoding, inventories,
retained Task count, Rust discovery/tail initialization or cursor state. Those
bounds, visible polling, complete capture, exact checkpoint Session navigation
and the configured human demo remain required in this Task/PR. This compression
pass establishes neither publication readiness nor Task completion.
