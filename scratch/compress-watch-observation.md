# Watch observation model compression — 2026-09-24

No further coherent model/API reduction found in HEAD `782dd3e75739` plus the
working observation-order feed. This pass adds only this report. Existing
implementation, tests, design edits and receipts remain intact.

## Model before and after

Unchanged: FlowPosition owns execution. Transactional Task events retain plans,
attempts, transitions and settlement. Rust projects those facts and attributed
Run manifests into TaskWatchSnapshot; TaskOutputPage independently pages
normalized output and source evidence. RegistryQuery transports both. Navigation
retains TaskWatchStore; mounted views own cancellable reads. Each TaskWatchOutput
keeps one chosen record revision with its display position and live precedence.
The store supplies observation ordinals; rows and contiguous groups are derived.

The working implementation already removed history/live order arrays, membership
sets, containsRevision and latestOutputSource. This compression pass removes no
additional type, field, command, DTO, persistence path or API.

## Path and mirrors inspected

Read `work/task/flow_history.rs`, the transactional history reader and position
receipt writer in `store/sqlite/durable.rs`, and the Watch/output read contracts
and discovery paths. Compared all Watch and output envelope fields with Swift
TaskWatch.swift and TaskOutput.swift, both shared JSON fixtures, and Rust/Swift
fixture assertions. Checked CLI declarations, read dispatch references and
RegistryQuery's typed reads and temporary cursor transport.

Traced WorkspaceNavigation and SessionsView into TaskWatchStore, the record
merge and row folding in TaskWatchOutput, and TaskWatchOutputView's grouping,
source evidence, filters and scroll anchors. Read the observation-order and
late-tool-completion tests, the working model diff and README. Searches found
none of the removed ordering fields/helpers, TaskOutputRecord or
TaskWatchStepKind in the inspected Watch paths.

## Candidates retained

- **Position, precedence and change order:** a record first loaded from history
  may later arrive live with the same revision. It must gain live precedence
  without becoming a new arrival. A later historical copy must not overwrite
  it. Display position, hasLiveRevision and lastLiveObservation therefore cannot
  derive from one another. Folding a tool completion preserves its original
  position while advancing its Follow target.
- **Sources, groups and row identity:** one source can produce several separated
  blocks. Groups are derived, not another retained transcript. Source record IDs
  deduplicate revisions; conversation IDs fold items; the composite row reference
  distinguishes identical conversation IDs in different Runs. Combining these
  would undo the tested A/B/A ordering or exact tool-update scrolling.
- **Latest page and accumulated records:** retaining TaskOutputSource also retains
  its latest page payload. A metadata-only mirror would introduce another wire
  shape without bounding the accumulated feed. Address this with the required
  retention design rather than clearing evidence fields or adding a wrapper.
  Likewise, caching derived groups would add ownership; it would not remove the
  need for bounded records and folding work.
- **History/live continuations and evidence:** their paging positions, unread
  flags and gaps remain independent. Combining them would lose failed-tail
  evidence or let historical paging control live continuation. outputRevision
  triggers scrolling when the same row changes again; the row reference alone
  cannot replace it.
- **Snapshot and output inventories:** bound attempts can lack manifests and
  auxiliary Runs can lack stages. Neither inventory derives the other. Selection,
  filtering, request identity and last-good errors also retain separate lifetimes.

## Verification and remaining work

No tests or builds rerun because executable content is unchanged. Inspected the
existing twelve-test model receipt and final three-test receipt, including two
render cases, under `watch-observation-evidence/`. All six production/test file
hashes match `source-hashes.json`. These are prior implementation receipts, not
fresh compression validation or configured interaction proof.

Bounded discovery, initialization and retention before visible polling, complete
capture, exact checkpoint Session navigation and the configured human demo
remain required in this Task/PR. This pass establishes no publication readiness
or Task completion.
