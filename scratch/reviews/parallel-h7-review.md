# H7 independent source review

2026-09-28 · LOO-298 · Bounded review for the managed implementation worker.
HEAD: `d07e569330c8dedceb5dd238ce9b22a7b6137006` (unchanged during review).
Only this note was written. No builds, tests, provider calls, installed-Home
inspection, Git mutations or Flow navigation ran. Findings below are source
counterexamples, not executed reproductions or configured acceptance.

## Confirmed source counterexamples

### 1. Explicit sync can erase the prose preserved by legacy adoption

`ops/pm.rs::pm_sync_async` (2441–2462) correctly adopts first, then normalizes
Project names with `LinearClient::update_project`. That operation
(`pm/linear.rs:908`) rewrites both description and full content from
`ProjectContent`; `pm/mod.rs::render_project_content` retains only targets,
Flow and KRs. Arbitrary authored prose and the existing summary disappear.

Reproduction input: migration-marked current Project named `previous` in Wave
A, with a custom description and content `Keep this prose.\n\n## Flows\nrecommended: custom`.
`canonical_project_name` accepts the bare name. Non-plan sync converts the Flow
correctly, then renames to `A — previous` and replaces the content, deleting
the prose. Clearing the adoption marker does not restore those bytes on retry.
This existing rename writer remains reachable through the new upgrade path.

Smallest proof: run explicit sync against the legacy provider fixture with that
bare name; assert the final name, exact retained description, and content equal
to the original with only `recommended:` replaced by `flow:`. Preserve custom
Flow and KRs. The existing adoption test uses canonical names and calls
`adopt_legacy_projects` directly; its exact-content assertion misses this path.
Name normalization should mutate the name without serializing unrelated content.

### 2. Rotation overwrites a cancellation it has already observed

`ops/chapter.rs::apply_rotation` checks predecessor status before processing
issues (654–668). Its later `checked_projects` result (758–770) rejects only a
third Started Project; it ignores the fresh predecessor/successor statuses,
then unconditionally writes predecessor Completed (771–774).

Reproduction input: ordinary old Started / next Planned rotation. After Task
dispositions settle, another writer sets old to Canceled. The final Project
enumeration returns old Canceled and next Started. Ownership still validates;
the code overwrites old with Completed and reports success. Alternatively,
return next Canceled at that enumeration: old is completed anyway, and only the
outer final check discovers that the Wave has no current Project.

Smallest proof: parameterize the existing provider fixture to change old or next
to Canceled immediately before that final enumeration. Assert rotation reports
the conflict, retains the observed statuses and does not complete old. Keep the
existing already-Completed retry case valid. This needs consumption of facts
already fetched, not a distributed lock or atomic provider transaction. The
twelve-mutation interruption test changes availability, not these observed facts.

## Reviewed preservation and retry boundaries

- `plan_rotation` uses the explicit target and stable IDs, accepts its partial
  status states, and leaves unrelated current names, duplicate targets and
  zero-current history without a shared predecessor unresolved. Those refusals
  are expected; this review proposes no newest-name or broad fallback rule.
- Deterministic `successor_id` now sets UUIDv4 bits. A lost create/attach reply
  leaves an ID that `find_project` can recover before another create. Activation,
  transfer, cancellation and completion each have provider read-back; repeat
  classification retains their already-applied results. The fixture applies each
  of twelve mutations before losing its reply and covers same/second stores.
- Planned successors retain their own Flow/KRs. The authored-successor assertion
  compares raw content after rotation; it does not accidentally copy predecessor
  Flow. New successors deliberately copy only the predecessor Flow.
- `move_chapter_task` updates only Project FK and timestamp, with same-Wave
  validation. The retained Task test compares identity, checkout, plan, PR and
  captured execution after transfer. Retirement commits locally before provider
  cancellation; its abandoned state remains retryable if that effect fails.
- `checked_projects_with_store` point-reads omitted locally known Project IDs;
  `rotation_tasks` does the same for locally retained predecessor Tasks.
  `ProjectNode::into_pm_project` maps archived history to Completed, preserving
  Canceled. The archive fixture retains Task/PR/capture and excludes that old
  Started provider record from current selection. Inaccessible point reads fail.
- Missing local Task evidence starts at `authored=None`: backlog/unstarted stays
  unresolved, while provider Started permits transfer and terminal issues remain
  history. Cancellation requires local negative evidence and the transaction's
  Started/claim/publication checks. Both recovery-fixture Homes explicitly seed
  clean local backlog checkouts; that is not proof of remote absence. Neither
  reader can recover an omitted identity that this Home has never observed.
- The draft preserves Project IDs, cached custom Flows and old current evidence
  before dropping `wave_chapters`. `legacy_current` scopes conversion; reads
  preview it, explicit adoption confirms provider state before clearing it.
  Archived Projects never promote; a fresh Started Project prevents old-current
  revival. Ordinary parsing has no permanent `recommended:` alias. The marker
  is migration evidence, not a new Chapter object.

No additional defect is established by this review. Arbitrary simultaneous edits
between provider reads/writes remain a concurrency hypothesis, distinct from
finding 2's ignored observed conflict. Executed Rust/materialized preservation,
full sync, configured Linear and truly separate-Home acceptance remain outside
this source review. Existing mocks establish only their authored scenarios.

## Inspected source fingerprints

Paths below are relative to `rust/loopflow/src/`; SHA-256 at final inspection.
Production hashes stayed unchanged. `chapter_tests.rs` changed concurrently:
`local_started_task` now rereads persisted timestamps before comparison. That
two-line fixture correction was inspected and does not address either finding.

| Path | SHA-256 |
| --- | --- |
| `ops/chapter.rs` | `459300946a8bfe786bb34d8a463d62a5e582d51b60d20a82bd0eb920d6a48187` |
| `ops/chapter_tests.rs` | `fc7e81a593fa399e47c5ae1d2f8f2d05fc9b4355fee2513e4ab7e4b067fa9ee1` |
| `ops/pm.rs` | `3ec2dfd7343f94b03c6fda98224972afe8c1e51920da8dca28ea74cc47264ff3` |
| `pm/linear.rs` | `05f7c4a8b7d3b6293fbbaec96833d9affd61a24a1f62d33362a1e1c5abb6097e` |
| `pm/mod.rs` | `4333f251ecc827bc5960f50f063e0af78a1e7656936e258d375b692d23101353` |
| `store/sqlite/chapters.rs` | `f22c0b4fadc73cf4681e7013e63a9e832ebd705c4c4797d0ef6b5fd8e95ab6af` |
| `store/migrations/drafts/project_status_chapters__9bea0388aa854e46a9db2d1e5c86ddfb.sql` | `e77ef75edea258d2b4a662a9e5df20ae967b07327fd390f14b5b6d747470e079` |
