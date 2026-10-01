# Task Sessions before landing

Jack Heart revised the landing contract on 2026-10-01. PR #1369 is open with
auto-merge disabled; landing stays stopped until this model is implemented.

A Task has any number of Sessions associated by its resolved checkout, including
subdirectories and symlink spellings. Repo and Wave Sessions retain their scopes
and are excluded. Session membership does not depend on waiting for participation,
readiness, or Flow membership. Attribution and completion authority stay separate.

Jack clarified that `session list --orphan` filters Sessions without Task
association. It is not a creation flag and cannot opt out of checkout association.
Desktop exposes this diagnostic inventory under a nested Debug menu, outside the
normal navigation. Repo and Wave conversations remain available in their own scopes.

Remove the branch's new Ask retry API and Ask-specific workspace affordances;
the broader Ask deletion is separate work. Preserve ordinary direct Flow Sessions.

Proof required: arbitrary Session count, canonical checkout association, repo/Wave
exclusion, orphan filtering and pagination, all Task Sessions visible without an
attention requirement, retained pane selection and splits. Update shared fixtures,
run focused Rust/Swift proof and both native build paths, then retry the configured
demo. Inspect existing CI failures before resuming landing.

The prior review and proof limits are in [the durable review](../docs/reviews/task-workspace.md).

## Implementation and proof

The implementation now records explicit repository/Wave scope in Session readings,
excludes those Sessions from Task workspace ownership, and derives Task IDs from
resolved checkout readings. Task/orphan filtering runs after resolution and before
pagination, so aliases cannot disappear at an SQL path-prefix filter. No placement
schema or scheduling behavior changed. Scope reads are batched.

Task/Wave indicators include all Task Sessions. Orphan access moved into the nested
diagnostic menu. The branch’s raw Ask retry key and caller-link expansion are removed;
existing upstream Ask lifecycle remains for its separate deletion.

Focused proof so far: 80 Swift tests, 32 human-session Rust tests, three Task membership
Rust tests, indexed metadata inventory test, architecture scan and 21 architecture
Python tests passed. Native fallback compilation, final CLI/golden checks, formatting and Clippy
with warnings denied passed. The final navigation refinement passed 29 tests. Configured provider and remote-Home proof remains unexecuted.

Review findings fixed: filtering before root resolution lost symlink Sessions;
reusing Task checkout identity for scoped Sessions could share their panes; the old
Ask caller lookup decoded captures during passive listing. Regression tests now
cover scope exclusions, aliases, stable orphan pages, and attention-independent counts.

Configured private-copy smoke: current CLI read three Sessions, all Task-associated,
and an empty orphan inventory. The reopened native app rendered planning and Task
Session counts without the old main-navigation orphan section. No provider Session
was opened by this check, and the copy contains no explicit repo/Wave primary rows;
those exclusion cases remain covered by local tests, not this live reading.

The demo helper needed its existing test-materialized 0.12.30 catalog restored after
source refresh; the first read rejected the mismatched catalog before app replacement.
The current branch source and matching test catalog then read the copied Home normally.
No main Home migration was attempted. Native screenshot: `/tmp/loopflow-20261001-162208.png`.

That capture exposed the accepted lowercase-name rule only reaching the older Wave
view model. Reuse the same `displayName` implementation in the new outline and title.
