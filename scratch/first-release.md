# LOO-486: first releases

Jack Heart requested first releases without a seed tag or version rollback,
preserving subsequent release ranges and retry/tag semantics (2026-10-14).
The Task directive supplies the accepted outcome; the implementation choices
below are local decisions, reconciled on 2026-10-14. Reviewable publication is
authorized and remains with delivery; landing requires a separate decision.

## Design

`release_commits_between(None, revision)` already collects first-parent history,
including the root commit, and applies target areas. The failure was routing
explicit `0.1.0` into `release_minor`'s closing-patch pair, which requires a
preceding cycle. An explicit version with no target tag now uses the
ordinary single-candidate lifecycle and its existing target lock. Relative
versions still require an explicit initial version. Existing minor cycles retain
their closing patch and baseline requirements.

Notes and previews carry an optional baseline through their existing context;
absence means repository start, never a synthetic Git ref. Explicit `--prev-tag`
still wins. Other targets' tags do not establish this target's baseline.

## Implemented reduction

`notes_base` now returns an optional baseline; `latest_tag` and
`release_commits_since` are deleted. Notes, preview and release execution share
`collect_changes_between`, including PR membership filtering. The existing
no-tag check test now states its actual proof: initial history is nonempty.

## Preservation and acceptance

- Public `lf release run 0.1.0` against a disposable bare remote, simulated notes
  provider and hosting: root changes included, notes archived, manifest already at
  0.1.0 accepted, only v0.1.0 tagged. Interrupt and repeat the same candidate.
- Initial notes/preview, named target area/prefix, and explicit override.
- Existing tagged patch and minor-cycle range/retry proofs remain intact.
- No live release, installed acceptance, reporting-repository mutations, or
  changes to PR #1555 and its completed-Task launch boundary.

## Evidence and remaining work

The disposable public CLI reproduced `a minor release needs a preceding release
cycle` at base `92b88800ec2b915bd0f1e4c2096262b727ab9659`. Configuration was not
required to trigger it. The selected implementation and deletion cut are complete.
The CLI regression now configures a hosted workflow, loses the dispatch reply,
then proves retry tags the same candidate without another release or closing patch.

Review: notes paths now share commit membership and PR filtering in
`collect_changes_between`, which already handles root commits and target areas.
The range algorithm and lifecycle remain unchanged. Missing minor-cycle baselines
still fail when target history exists; historical preview inference is unchanged.
No additional implementation mismatch was found in reconciliation. Local
`origin/main` has no commits beyond the checkout base; this is not a fresh remote
observation. The infrastructure objective and complete release-child memory
retain operation-entry proof and same-version recovery as relevant lessons.

Checks: network-isolated `release_tests` (72 passed), `integration release_lock_tests::` (15 passed), and library `ops::release::tests::` (41 passed); `scripts/check_architecture.py` and `git diff --check` pass; prior `cargo fmt` and `cargo clippy --all-targets -- -D warnings` passes apply to unchanged Rust content; full materialized matrix remains with CI.

Publication review found no further implementation gap. The Wave memory edits
remain local because that file also contains unrelated planning-history changes.
The release and lock suites are the affected acceptance checks; the full
materialized CI matrix remains separate. Remaining work is reviewable PR publication; landing requires a separate decision.
Hosting and note generation were simulated: the tests establish input membership,
archival and candidate identity, not generated prose quality, live publication or
installed acceptance. Historical preview inference is unchanged. No product
decision blocks this repair; landing remains outside the publication authority.
