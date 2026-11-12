# LOO-857 plan

Cause: `scratch/linear-same-revision-conflict-5whys.md` (Linear shortened
`branchName` to ≤60 chars between 2026-11-06 and 11-08 without bumping
`updatedAt`; `put_item` refused the equal-revision read forever).

Built:
- `put_item` (`linear/acceptance.rs`): no equal-revision refusal; reads order
  by `(revision, observed_at)`, so at one revision the later read wins.
  Pending `task_changes` are untouched, so local edits stay queued.
- Messages name real remedies: Task errors `lf task sync <issue>`
  (`ops/linear.rs`), Project errors "edit it in Linear / add it back, then run
  `lf wave sync`".
- `docs/work.md` (Linear sync section): the equal-`updatedAt` behavior and
  which command each error names.
- Deleted unused `StoreError::ProviderObservationConflict`.
- Tests: `shortened_branch_name_at_the_same_revision_replaces_the_stored_one`,
  equal-revision cases in `acceptance.rs` and `planning_lookup_tests.rs`
  rewritten to the later-read rule.

Left as is: Project equal-revision conflict in `put_project` (name-cutover
design, `project_name_cutover_retains_both_histories_and_rejects_later_conflicts`).
See questions.md.

Checks:
- `cargo test -p loopflow --lib -- linear::acceptance:: ops::linear::planning_lookup_tests::` — pass (implement, compress).
- `cargo clippy --all-targets -- -D warnings`, `cargo test -p loopflow --test integration documented_commands::` — pass (implement). Gate runs affected suites.
