# v0.12.18

<!-- loopflow:release-notes=narrative;gate=safe -->

v0.12.18 makes chapter changes easier to assess and safer to reset. New chapter workflows review KR evidence across repository, project, and Wave scopes, while interactive starts preserve human direction through scoped Runs and apply only the accepted plan. Terminal skill catalogs also stay aligned without changing redirected output.

## Review and reset plans as chapters

Chapter work now has built-in start and review paths at repository, project, and Wave scope. Reviews can judge KR progress autonomously; starts capture human direction before scoped Runs, reconcile the resulting challenges, and carry the accepted plan forward.

- Six built-in chapter skills cover starting and reviewing work at all three scopes.
- Repository chapter starts record human Wave direction before launching scoped Runs.
- Interactive starts reconcile the Runs' challenges and apply only the plan the human accepts.
- Bound and nested Runs leave their edits uncommitted for the owning Task to inspect and checkpoint.

## Small changes

- `lf list` now returns to column zero on each terminal newline, preventing skill catalog output from drifting right in raw terminals.
- Redirected and captured `lf list` output keeps its original LF bytes.
