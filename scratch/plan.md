# LOO-857 plan

Cause: `scratch/linear-same-revision-conflict-5whys.md` (Linear shortened
`branchName` to ≤60 chars between 2026-11-06 and 11-08 without bumping
`updatedAt`; `put_item` refused the equal-revision read forever).

Built:
- `put_item` (`linear/acceptance.rs`): no equal-revision refusal; reads order
  by `(revision, observed_at)`, so at one revision the later read wins.
  Pending `task_changes` are untouched, so local edits stay queued.
- Messages name real remedies: Task errors `lf task sync <issue>`
  (`ops/linear.rs`); the remaining Project error (missing from its
  Initiative) says "add it back, then run `lf wave sync`".
- `docs/work.md` (Linear sync section): the equal-`updatedAt` behavior and
  which command each error names.
- Deleted unused `StoreError::ProviderObservationConflict`.
- Tests: `shortened_branch_name_at_the_same_revision_replaces_the_stored_one`,
  equal-revision cases in `acceptance.rs` and `planning_lookup_tests.rs`
  rewritten to the later-read rule.

- `put_project` follows the same rule (concept review, 2026-11-12: Tasks
  and Projects are one concept, a Linear observation). The equal-revision
  refusal, its "edit it in Linear" message and the name-cutover gate
  (`pending_name_cutover` query, `converted_at` update) are gone; the
  `pm_project_name_cutover` table stays (dropping it is a staged contract
  migration). Description COALESCE stays. Test:
  `project_reads_at_one_revision_keep_the_later_one`;
  `rejected_project_snapshot_preserves_durable_project_facts` now rejects on
  the Initiative check after accepting a same-revision rename.

Checks:
- `cargo test -p loopflow --lib -- linear::acceptance:: ops::linear::planning_lookup_tests::` — pass (implement, compress).
- `cargo clippy --all-targets -- -D warnings`, `cargo test -p loopflow --test integration documented_commands::` — pass (implement). Gate runs affected suites.
