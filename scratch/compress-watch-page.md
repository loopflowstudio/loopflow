# Normalized Watch paging compression — 2026-09-24

No coherent model/API reduction found in HEAD `4fe578a5a894` plus the working
normalized-page bounds. This pass adds only this report; existing implementation,
tests, design edits and evidence remain intact.

## Model before and after

Unchanged: FlowPosition owns execution. Transactional Task events retain plans,
attempts, transitions and settlement. Rust projects those facts and attributed
Run manifests into TaskWatchSnapshot. Journals and native history own output;
SourcePage applies one normalized-record budget across their readers, while
TaskOutputPage adds attribution and source evidence. Private cursors retain
reader progress. RegistryQuery transports the public envelopes and opaque
continuations. TaskWatchStore retains inspection and independent history/live
reads; TaskWatchOutput keeps a bounded window of chosen record revisions and
derives display rows.

No command, DTO, field, type, persistence path or API was removed or collapsed.

## Path and mirrors inspected

Read `work/task/flow_history.rs`, the transactional history read and position
receipt writer in `store/sqlite/durable.rs`, Watch/output projection contracts,
and manifest discovery in `run_record.rs`. Followed JSONL continuation, tail
initialization, native normalization, OpenCode revision admission and the shared
page budget through `run_record/output.rs` and `output/native.rs`.

Compared Rust Watch/output fields with Swift `TaskWatch.swift` and
`TaskOutput.swift`, and inspected both languages' shared-fixture assertions.
Followed CLI Watch/Output declarations through RegistryQuery's typed reads and
temporary cursor transport, then TaskWatchView, TaskWatchStore and the output
merge/retention model. Read the active design, working diff and paging proof.
Source searches found no remaining TaskOutputRecord, TaskWatchStepKind,
containsRevision or latestOutputSource names in the inspected Rust/Swift paths.

## Candidates retained

- **Byte offset, block index and line digest:** offset locates a source record;
  index resumes normalization inside it. The digest detects replacement of that
  pending record, which the preceding-byte anchor cannot detect. Combining them
  would lose continuation or reset evidence. Provider-specific cursor variants
  would rearrange these facts without removing an owner or bounding inventory.
- **Source, page and retained-window budgets:** source limits bound input work;
  normalized limits cover expansion from one input into many records; desktop
  retention bounds accumulated output across requests. They govern different
  allocations. The page's cached byte count avoids serializing every accepted
  record again for each admission; it is neither another payload nor a wire field.
- **Normalization and admission:** native `push` constructs a conversation event
  and revision; SourcePage admits the resulting OutputRecord for every reader.
  A full page leaves a record unread, while an individually oversized record is
  consumed with a gap. A generic budget wrapper would add vocabulary without
  consolidating another owner. `has_more` also covers ignored source records and
  partial-message work, so it cannot derive solely from output count.
- **OpenCode progress and revision memory:** keyset position, frozen timestamp
  ceiling and retained revision hashes prevent distinct loss/replay cases. An
  unread revision must not become seen. JSONL block continuation cannot replace
  these mutable-part semantics.
- **Reader and public envelopes:** SourcePage carries successful continuation;
  TaskOutputSource also represents attributed unavailable sources. Watch attempts
  can lack manifests, and auxiliary output can lack stages. Neither inventory
  derives from the other. Desktop history/live precedence, display position and
  retention recency likewise preserve different behavior; merging them would
  change ordering or recovery.

## Verification and remaining work

No tests or builds rerun because executable content is unchanged. All eight
entries in [the existing hash manifest](watch-page-evidence/hashes.json) still
match, including the three Rust files, Swift consumers, integration probe and
branch CLI. Inspected the receipts for 14 passing Rust reader tests, one passing
desktop integration test and Clippy. These remain prior implementation evidence;
the desktop proof uses synthetic provider storage, not configured live capture
or physical UI interaction. `git diff --check` passes.

Discovery, tail initialization, inventory/cursor growth and incoming allocation
bounds remain required before automatic polling. Complete capture, exact
checkpoint Session navigation, configured end-to-end proof and the human demo
remain required in this Task/PR. No publication or Task completion is established.
