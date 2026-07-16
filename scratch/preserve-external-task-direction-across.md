# Preserve external Task direction across the succession boundary

## Problem

When a Task Session terminates (completes or abandons), re-running `lf task run
<issue>` seeded the successor from the Linear launch snapshot. That swallowed the
predecessor's already-applied edits and re-emitted them on the successor, while a
racing webhook redelivery of an already-ingested comment became a duplicate
follow-up. The predecessor's worktree and branch also still occupied disk, so
`plan_placement` returned a non-`Create` strategy and `task_run` errored before
the successor could ever be placed.

## Design

A terminal Task Session keeps its row for attributable history and leaves its
**direction** — the Linear observation cursor and the ingested-comment ledger —
to a single successor, carried in one transaction with successor creation.

- **Carry transaction** (`reserve_task_session_successor`): inserts the successor
  Session + sequence-1 PR + initial directive, then re-keys (moves, not copies)
  `task_linear_observations` and `task_linear_ingested_comments` from predecessor
  to successor. Seed-if-absent handles a predecessor predating the cursor
  migration. Historical receipts (`child_commands`, `child_directives`) stay on
  the predecessor.
- **Resolution** (`task_session_by_issue`): one Linear issue may now have a
  terminal predecessor plus one non-terminal successor. Partial unique indexes
  (`idx_task_sessions_one_current_issue` / `_identifier` / `_worktree`) guarantee
  at most one non-terminal Session per issue. Resolution prefers non-terminal,
  then newest terminal — never errors on count > 1.
- **Placement**: the successor needs a distinct worktree/branch because the
  terminal predecessor's may still occupy disk. `succession_workspace_slug`
  derives `<base>-s<tail8>` from the predecessor's id (base capped at four words
  so the suffix word stays within the 2-5 word limit). `task_run` captures the
  terminal predecessor's id in the early check and uses it for slug derivation
  before `plan_placement`.
- **Idempotency**: `non_terminal_successor_for_issue` is the probe. A crash or
  concurrent run that already created the successor returns it with
  `created: false`; no re-key runs again.
- **Soft link**: `predecessor_session_id` column on `task_sessions` records which
  terminal Session's direction a successor carries. Written by the carry
  transaction; not yet surfaced in the `TaskSession` struct (see questions.md).

## Migration

`0.11.018_task_session_successors.sql` rebuilds `task_sessions` (SQLite bakes
UNIQUE into the table, so the issue_id / issue_identifier / worktree UNIQUEs
require the documented table rebuild, same shape as 0.11.002 for Project
Sessions), adds the three partial unique indexes, and `ALTER TABLE` adds
`predecessor_session_id`.

## Proof

`tests/task_session_succession_tests.rs` — two integration tests:
- `succession_carries_direction_and_racing_recovery_is_exactly_once`: the core
  contract (carry, exactly-once racing recovery, receipts stay attributable,
  idempotent retry).
- `webhooks_resolve_to_the_successor_across_the_boundary`: the webhook
  integration boundary.

`ops::task::tests::succession_slug_is_distinct_capped_and_per_predecessor`: the
slug derivation unit test.
