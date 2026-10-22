# v0.12.32

<!-- loopflow:release-notes=narrative;gate=safe -->

v0.12.32 helps operators recover Tasks stalled by unrelated execution history or a missing GitHub PR link. Recovery preserves the Task’s history, checkout and unfinished work, while PR reconciliation can rediscover an existing delivery and record authoritative merge evidence.

## Recover workers without discarding history

Unrelated historical Execs with missing process receipts no longer prevent Task worker recovery. Evidence of current work still controls whether recovery can proceed.

- Live processes, unresolved current Session drivers and Flow claims still block recovery.
- Task completion and cleanup retain stricter checks; recovering a worker does not authorize removing unfinished work.

## Restore missing PR links

Run `lf pr reconcile` when a Task has lost its GitHub PR identity. Reconciliation uses the Task’s recorded branch to discover an existing PR and recover its delivery evidence.

- A unique matching PR restores the GitHub identity and, when merged, records authoritative merge evidence.
- Missing or ambiguous matches and failed provider reads preserve local work and leave delivery unresolved.
- Ordinary status checks do not search unpublished branches. Reconciliation keeps the Task open and local edits intact.

## Operational notes

The supplied change records passing reconciliation, Task-work and Session-record coverage, the scorecard regression, formatting and all-target Clippy. Installed worker recovery and continuation, including the missing-link reconciliation walkthrough, still need end-to-end verification.