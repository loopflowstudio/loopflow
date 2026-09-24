# Watch continuation compression — 2026-09-24

No executable reduction. Reviewed HEAD `5fcf327dbcce` and the working independent
tail-start implementation. This pass adds only this report; existing edits and
the concurrently present `review-tail-proof.py` remain untouched.

## Model before and after

Unchanged: FlowPosition authorizes execution; committed Task events retain the
expanded plan and stage/attempt facts. Rust projects those facts and attributed
Run manifests into TaskWatchSnapshot. Journals and provider-native history own
output. TaskOutputPage groups normalized records with Run attribution and source
evidence. Private cursors retain reader progress. History and live starts produce
the same continuation format and use the same reader. RegistryQuery transports
opaque cursors; TaskWatchStore retains only snapshot inspection state.

No command, DTO, field, type, persistence path, or API was removed or collapsed.

## Path and mirrors inspected

Read `work/task/flow_history.rs`, transactional history reads and position receipt
writing in `store/sqlite/durable.rs`, `ops/task_watch.rs`, and
`ops/task_output.rs`. Followed manifest discovery and native source resolution
through `run_record.rs`, `run_record/output.rs`, and `output/native.rs`, including
JSONL tail seeding, journal mode reconstruction, and OpenCode revision retention.
Traced `task output` through both CLI dispatch paths, the Task operation, and
RegistryQuery's temporary cursor-file transport.

Compared the Rust Watch/output DTOs field-by-field with Swift TaskWatch.swift and
TaskOutput.swift, both shared JSON fixtures, and their Rust/Swift assertions.
Read TaskWatchStore and the view's refresh/selection path, the large-cursor
transport test, README examples, and the active design. Source searches found no
remaining TaskOutputRecord or TaskWatchStepKind compatibility names.

## Candidates retained

- **Start mode and continuation:** `--tail` chooses an initial position only;
  continuation already has one path. A new public request enum would add a name
  without removing an owner or implementation. There is no second live cursor
  schema or live-output command to delete.
- **Inventory seeding and page reads:** all existing sources must be seeded
  before later round-robin visits, or those visits could skip intervening output.
  Source resolution is already shared. Combining the loops without a complete
  bounded-discovery design would change that guarantee.
- **OpenCode boundaries and unfinished revisions:** watermark, frozen ceiling,
  page position, and retained hashes prevent different loss/replay cases. Older
  unfinished IDs must survive a completion at an unchanged timestamp. Keeping
  only the maximum timestamp or querying current unfinished rows loses output.
- **JSONL identity and capture mode:** location, file identity, byte anchor,
  Session verification, and attempt capture mode establish different facts.
  Tail-window uncertainty also differs from a known attempt with no output yet.
  Replacing these with a generic offset or deleting the uncertainty state would
  weaken source/reset evidence or permit duplicate summary output.
- **Provider-specific cursor fields:** a tagged private cursor could exclude
  irrelevant fields, but would add variants and serialization work without
  removing a reader or solving retained-state growth. Revisit with the required
  bounded-state implementation rather than making an isolated shape change.
- **Snapshot versus output:** attempts may lack manifests; auxiliary Runs may
  lack stages. Output pages need independent labels and availability. Combining
  the projections or deriving one inventory exclusively from the other loses
  evidence. UI selection, stale snapshots, and request identity likewise have
  separate lifetimes; no output cache or second reducer was introduced here.

## Verification and remaining work

No tests or builds rerun because this pass changes no executable content.
Inspected the existing receipts: three reader tests in
`/tmp/loo293-tail-readers-final.log`, one discovery test in
`/tmp/loo293-tail-discovery-final.log`, two Swift tests in
`/tmp/loo293-tail-swift.log`, and the completed clippy log. These remain prior
implementation receipts, not fresh compression validation. The four reviewed
reader/query file hashes still match the supplied workspace snapshot.

`watch-tail-proof.json` records available but quiet configured sources, not live
arrival or UI proof. Complete capture, bounded discovery/initialization/state,
feed merging/filtering/Follow live, connected diagrams, exact human Session
navigation, and the configured human demo remain required in this Task and PR.
This review establishes no publication readiness or Task completion.
