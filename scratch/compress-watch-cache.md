# Recent Watch retention compression — 2026-09-24

No further coherent model/API reduction found in HEAD `3a00c7826` plus the
working four-Task retention change. This pass adds only this report; existing
implementation, tests, design edits and proof notes remain intact.

## Model before and after

Unchanged: FlowPosition owns execution. Transactional Task events retain plans,
attempts, transitions and settlement. Rust projects those facts and attributed
Run manifests into TaskWatchSnapshot; TaskOutputPage independently carries
normalized output, source evidence and continuation. RegistryQuery transports
both. WorkspaceNavigation retains four recent TaskWatchStores per repository
and window. Mounted views own cancellable reads. Each store retains inspection,
independent history/live continuations and a bounded record/payload window;
display rows and contiguous groups are derived.

No command, DTO, field, type, persistence path or API was removed in this pass.
The working implementation already replaced the unbounded dictionary with one
ordered collection, without adding a separate recency index or cache owner.

## Path and mirrors inspected

Read TaskFlowStage/Event, the transactional history read and position receipt
writer, Watch projection, output read/continuation contract, manifest discovery
and source-reader envelopes. Compared Rust/Swift Watch and output fields, both
shared JSON fixtures and their fixture assertions. Followed the shared read-only
Task lookup and CLI declarations through RegistryQuery's typed reads and temporary
cursor transport.

Traced per-repository navigation through SessionsView into TaskWatchView,
TaskWatchStore and TaskWatchOutput; checked the separate TaskWorkspaceView store.
Read the working retention test, README delta and active design. Source searches
found none of TaskOutputRecord, TaskWatchStepKind, containsRevision or
latestOutputSource in the inspected Rust/Swift paths.

## Candidates retained

- **Cache key and snapshot identity:** navigation must find a store before its
  first successful read, including after failure. Snapshot identity cannot own
  that lookup. The ordered collection already expresses membership and recency
  together; adding a cache type would move this ownership without reducing it.
- **Store count and output budget:** four retained presentations and each
  presentation's record/payload window bound different allocations. Merging
  these policies would change eviction and recovery behavior. Terminal surfaces
  also have different lifetimes: releasing them ends their PTYs.
- **Navigation and sheet stores:** both use the same presentation model, but
  have different view lifetimes. A global store would couple independent windows
  and inspection state. No second selection cache or eviction tombstones exist.
- **Request identity and mounted cancellation:** cancellation follows view
  lifetime; request IDs reject superseded responses. Neither replaces the other.
  Last-good evidence must also coexist with read errors.
- **History/live state and record positions:** continuation, revision precedence,
  display position, latest live change and retention recency preserve different
  behavior. Reopening an evicted store intentionally discards all of them and
  recovers through existing readers; retaining a smaller hidden cache would undo
  that ownership reduction.
- **Snapshot and output inventories:** auxiliary Runs may have no stage, and
  bound attempts may lack readable manifests. Neither inventory derives from the
  other. Source availability, reset and gaps likewise cannot collapse into empty
  records. Clearing the latest page's records already leaves one payload owner.

## Verification and remaining work

No tests or builds rerun because executable content is unchanged. Inspected the
existing two-test receipt at `/tmp/loo293-watch-cache-tests.log`; both navigation
source and test SHA-256 values still match the supplied workspace snapshot.
This remains prior fixture/model/view evidence, not fresh configured interaction
or measured app-memory proof. Whitespace checked with `git diff --check`.

The cache bounds retained store count per repository/window only. Source and
snapshot inventories, incoming decoding, Rust discovery/tail initialization and
cursor growth still need bounds before visible polling. Complete capture, exact
checkpoint Session navigation and the configured human demo remain required in
this Task/PR. No publication, landing or Task completion is established here.
