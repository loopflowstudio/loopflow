# Recover missing Task delivery evidence

Jack Heart requested recovery of Infrastructure's blocked workers. LOO-295's
PR #1283 is merged, but its Task PR row has no GitHub identity. Current
reconciliation skips that row and publication could create a duplicate PR.

Make explicit `lf pr reconcile` in a Task checkout discover its existing PR by
the recorded branch when the number is absent, then reuse normal authoritative
merge settlement. Ordinary cached status must not enumerate unpublished PRs.
Missing, ambiguous and failed remote reads preserve local work and uncertainty.
Reconciliation must not publish, rotate, complete the Task or remove its dirty
line-count viewer. Its separate live continuation proof remains outstanding.

The same repair carries LOO-367's tested admission correction: resume uses
current execution ownership rather than unrelated historical Exec uncertainty.
Live processes, unresolved current Session drivers and Flow claims still block;
completion, abandonment and checkout restoration keep their stricter checks.
The original fix remains preserved in LOO-367's checkout; this copy allows the
installed recovery to ship without prematurely delivering that Task's other work.

Check: reconciliation regression (four discovery cases) and four task-work tests
passed; cargo clippy --all-targets -- -D warnings and cargo fmt --check passed.

Review: remote discovery is explicit and bounded to two candidates; ambiguous
and failed reads retain local state. Ordinary status keeps its numbered/cached
path. Reconciliation preserves PR identity, keeps Work open, and does not touch
the authored viewer. Installed recovery requires publishing and installing the
repair; no branch binary may open the production Home. LOO-295's configured
continuation proof remains an independent obligation after association recovery.
