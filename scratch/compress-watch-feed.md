# Watch feed compression — 2026-09-24

No coherent executable reduction found in HEAD `2056f4a62` plus the working
desktop feed. Existing implementation and test edits remain intact. This pass
adds this report and corrects stale shared-read descriptions in the active
design: the snapshot has no history cursor, independent output continuation is
implemented, and desktop reads remain manual.

## Model before and after

Unchanged: FlowPosition owns execution. Transactional Task events retain the
expanded plan, exact attempts, transitions and settlement. Rust projects those
facts and Run manifests into TaskWatchSnapshot. Journals and native provider
history own output; TaskOutputPage carries attributed normalized records and
source evidence. RegistryQuery transports opaque continuations. Navigation
retains each TaskWatchStore within a window/repository; the mounted view owns
cancellable reads. TaskWatchOutput merges loaded records and derives display
rows. None of these presentation values author durable history or Sessions.

No command, DTO, field, type, persistence path or API was removed or collapsed.

## Inspected path and mirrors

Read `work/task/flow_history.rs`, the transactional history read and receipt
writer in `store/sqlite/durable.rs`, and the Watch/output projections. Checked
the output reader's public record/source/cursor shapes and initial tail path.
Followed the shared read-only Task lookup through CLI Watch/Output declarations
and dispatch, then RegistryQuery's typed reads and temporary cursor transport.

Compared Rust/Swift Watch and output DTO fields, both shared JSON fixtures and
their fixture assertions. Traced WorkspaceNavigation and the conditional
SessionsView destination into TaskWatchStore, TaskWatchOutput, TaskWatchView
and TaskWatchOutputView. Read the feed/row behavioral tests, updated inspection
tests, README and accepted/current design. Source searches found no surviving
TaskOutputRecord or TaskWatchStepKind aliases.

## Candidates retained

- **History/live state:** independent cursors, order, membership, paging and
  gaps serve different reads. The order arrays preserve reader order; sets
  deduplicate membership and establish live revision precedence; the record map
  retains one selected revision. Removing sets creates repeated linear scans.
  Moving them into a generic lane wrapper adds a type without removing an owner.
  A single gap array would let history erase failed live-start evidence.
- **Page versus accumulated output:** TaskOutputSource describes the latest
  source observation; TaskWatchOutput describes accumulated display evidence.
  Its retained page also carries records already represented in the merge map.
  A second metadata-only mirror would duplicate the wire contract without
  solving the unbounded retained feed. Revisit that representation with bounded
  retention and contiguous cross-Run grouping as one coherent change.
- **Record versus display identity:** source IDs and revisions deduplicate
  reader output. Conversation item IDs fold starts, deltas and final snapshots.
  Row kind and contiguous text identity prevent collisions and keep prose on
  the correct side of intervening tools. Replacing either identity with the
  other loses behavior already covered by the feed tests.
- **Selection, filter and follow:** the visible plan selection can follow the
  active stage while output includes every Run. A Run filter can further narrow
  a stage filter; scrolling pauses following without changing either selection.
  Combining these into one selected-stage value would remove useful states.
- **Follow target versus revision:** the most recently changed source chooses
  the scroll destination; outputRevision triggers scrolling when that same
  source changes again. Source count or last Run cannot replace either value.
- **Request versus view lifetime:** view refresh state starts cancellable work;
  store request IDs reject superseded responses after navigation or overlapping
  reads. Snapshot/output errors preserve independent last-good evidence. A
  shared request wrapper would relocate these distinctions, not remove them.
- **Snapshot versus output inventory:** bound attempts can lack manifests and
  auxiliary Runs can lack stages. Deriving either projection solely from the
  other loses those cases. Source health, reset, paging and capture gaps likewise
  cannot be inferred from an empty records array.

## Verification and remaining work

No tests or builds rerun: executable content is unchanged. Inspected the prior
14-test pass in `/tmp/loo293-feed-verified.log`, including both rendering cases.
The four feed production files and three test files still match the SHA-256
values supplied with this iteration. This is an existing implementation receipt,
not fresh compression validation or configured Watch proof. Whitespace checked
with `git diff --check`.

Bounded discovery/initialization/retained state, automatic visible polling,
complete capture, contiguous cross-Run observation ordering, connected diagrams,
exact checkpoint Session navigation and the configured human demo remain in
this Task and PR. This pass establishes no publication or settlement readiness.
