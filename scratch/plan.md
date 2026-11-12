# LOO-857 plan

Cause: `scratch/linear-same-revision-conflict-5whys.md` (Linear shortened
`branchName` to ≤60 chars between 2026-11-06 and 11-08 without bumping
`updatedAt`; `put_item` refused the equal-revision read forever).

Built:
- `put_item` (`linear/acceptance.rs`): no equal-revision refusal; reads order
  by `(revision, observed_at)`, so at one revision the later read wins.
  Pending `task_changes` are untouched, so local edits stay queued.
- Messages name real remedies: Task errors `lf task sync <issue>`
  (`ops/linear.rs`); the two remaining Project errors (missing from or
  moved off its Initiative) end "then run `lf wave sync`".
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

- Project membership (concept review 2, 2026-11-12):
  `validate_project_membership` checks only the Initiative. Team ids follow
  `put_project`'s later-read rule, and an accepted `put_project` write clears
  `membership_unresolved` (it passed the Initiative check, so a held move that
  Linear reversed heals on the next read). An Initiative difference at a
  revision that isn't older, unrevisioned reads included, still sets
  `membership_unresolved=1`; message "Project X moved to another Initiative in
  Linear; move it back, then run `lf wave sync`". `reconcile_linear_project_teams_in`
  keeps its forced Team write. Deleted `StoreError::ProjectMembershipConflict`.
  Launch and completion refusals (`waves.rs`, `children.rs`) name
  `lf task sync <identifier>`. Tests: `team_change_at_the_stored_revision_follows_linear`;
  the `planning_lookup_tests` membership test renamed to the Initiative move;
  `project_field_delivery_requires_revision_accepted_into_work` drops its
  "unresolved" Team case (a re-read at the original Initiative now heals it;
  "foreign" covers a move that persists).

Checks:
- `cargo test -p loopflow --lib -- linear::acceptance:: ops::linear::planning_lookup_tests:: project_field_delivery_requires_revision_accepted_into_work` — pass (implement; compress made no code edits).
- `cargo clippy --all-targets -- -D warnings`, `cargo test -p loopflow --test integration documented_commands::` — pass (implement). Gate runs affected suites.
