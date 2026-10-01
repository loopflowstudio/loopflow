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
