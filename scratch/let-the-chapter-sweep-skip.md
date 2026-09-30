# Skip foreign-Team Projects (LOO-359)

Jack Heart requested this repair on 2026-09-30 after Technical Architecture,
a foreign-Team Project in Infrastructure's Initiative, blocked the entire sweep.

## Design

Filter Projects with known Teams that exclude the repository Team before read
validation, title normalization, issue enumeration, and sync rename planning.
Preserve strict ownership validation for missing Teams, shared repository/foreign
Teams, and explicit mutations. Retained Project/Task history remains intact.
Sweep reports each skipped Project once by stable ID, including JSON output;
Project-level entries have no issue. Eligible repository issues keep the existing
preview, exclusion, fresh-membership check, and cancellation path.

## Delete — do not maintain

Remove the unconditional foreign-Team rejection from Project enumeration in
sweep, planning refresh, and wave sync. Keep the ownership validator for mutations
and repository-owned snapshot integrity. No schema migration or parallel owner.

## Remaining acceptance

- Gate owns the affected Task-planning suite against the materialized migration
  graph. The focused fixture uses the normal hermetic store with embedded drafts.
- Configured acceptance still needs a live preview naming Technical Architecture
  once and cancellation of eligible LOO-309/LOO-329 issues if still open. No live
  issue was canceled during implementation.

## Review — 2026-09-30

Sync's second Project enumeration also needed filtering: fixing only its preview
would have allowed foreign Project renames. That loop now skips foreign ownership
and validates remaining ownership before renaming. Explicit Task mutations retain
their strict validator. Snapshot refresh only upserts observed Projects/Tasks;
omitting a foreign Project does not delete retained history. Sweep's existing
fresh membership and worker/PR exclusions remain unchanged.

Check: `cargo test -p loopflow --lib foreign_projects_do_not_block_sweep_refresh_or_sync -- --nocapture` passed (1); `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and `git diff --check` passed; materialized affected suite and configured acceptance belong to gate/demo.
