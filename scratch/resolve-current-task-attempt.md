# W2-249 — Resolve the current Task attempt consistently after recovery

## Directive (v1, the contract)

Centralize current Task Session resolution once terminal predecessors and active
successors can share an issue and worktree. Issue, identifier, and worktree
lookups used by PR publication, stacking, team rebinding, status/roadmap, and
Project supervision must select the unique live/current successor while
preserving history.

Done when:
1. terminal predecessors never win an operational lookup
2. shared-worktree recovery histories do not cause duplicate-worktree errors
3. identifier rebinding updates the current successor
4. ambiguity or multiple live successors fails actionably
5. deterministic store and CLI tests cover every consumer

## Problem

`task_sessions` bakes column-level `UNIQUE` into `issue_id`, `issue_identifier`,
and `worktree` (migration 0.11.001). So a terminal predecessor and a live
successor can never coexist — the second insert hits the unique constraint
("duplicate worktree"). `task_session_by_issue` errors on >1 row; there is no
notion of "current successor." `rebind_task_issue_identifier` uses `query_row`
on `issue_id`, which breaks the moment two rows share an issue.

Project Sessions already solved this (0.11.002): the column `UNIQUE` was dropped
and replaced with a partial unique index `WHERE status NOT IN ('completed',
'abandoned')`, keeping terminal rows as history while allowing one current
successor. Task `is_terminal()` is `Completed | Abandoned` (failed is live and
resumable, so it holds the current slot — you resume a failed task, you don't
succeed it).

## Design

### Schema (migration 0.11.018_task_session_successors)

Rebuild `task_sessions` without column `UNIQUE` on `issue_id` /
`issue_identifier` / `worktree` (keep `id` PRIMARY KEY), then add three partial
unique indexes mirroring 0.11.002:

```sql
CREATE UNIQUE INDEX idx_task_sessions_one_current_issue
    ON task_sessions(issue_id) WHERE status NOT IN ('completed', 'abandoned');
CREATE UNIQUE INDEX idx_task_sessions_one_current_identifier
    ON task_sessions(issue_identifier) WHERE status NOT IN ('completed', 'abandoned');
CREATE UNIQUE INDEX idx_task_sessions_one_current_worktree
    ON task_sessions(worktree) WHERE status NOT IN ('completed', 'abandoned');
```

Recreate `idx_task_sessions_wave_status`. A failed session still occupies the
slot (failed is live); only completed/abandoned free it for a successor.

### Centralized resolution

One helper, used by every consumer:

```text
resolve_current_task_session(sessions) ->
  live = sessions where !status.is_terminal()
  if live.len() > 1  -> Err(ambiguous, list id+identifier+status)
  if live.len() == 1 -> Ok(that live successor)
  if live.is_empty() -> Ok(most recent terminal by (updated_at, created_at))
```

The live successor wins; a terminal predecessor is returned only when no live
successor exists (so `task status`/`complete`/webhook reads on a completed Task
still resolve it — preserving history). `task_session_by_issue` and
`task_session_by_worktree` both route through this helper — that is the
"centralize." Multiple live successors is actionable ambiguity, never a silent
pick. (`by_issue` matches `issue_id = ? OR issue_identifier = ?`, so a live row
matched by id and a different live row matched by identifier is the cross-match
ambiguity the partial indexes cannot prevent alone.)

### Rebind (criterion 3)

`rebind_task_issue_identifier` resolves the current successor by `issue_id`
through the same rule, then rebinds THAT row. Order preserved: `==new` →
`Ok(false)` (idempotent); `!=old` → error; active-body guard; update by `id`.

### Scope decision (executive)

`task_run` keeps its current reuse semantics (it does not start creating
successor rows). The directive is about *resolution*, not the creation flow:
"Done when" is entirely lookup/test behavior. Introducing successor-creation
here would add untested behavior and weaken the contract. The schema now
*allows* sharing (criterion 2), and resolution is correct *when* sharing occurs
(criteria 1, 4) — exercised by tests that insert terminal+live pairs directly.

## Consumers covered (criterion 5)

`get_task_session_by_issue`: task_run reuse (×2), stack parent, reservation
recovery, task_status, task_complete, task_steer/interrupt/etc., pm Project
supervision (reteam), webhook PR publication.
`get_task_session_by_worktree`: task_stack (stacking).

## Tests

Store (deterministic, in-memory): migration preserves history + child refs and
enforces one-current; resolution picks live over terminal, terminal fallback,
ambiguous-live errors; rebind updates the live successor, idempotent, active-body
guard, terminal-fallback target.
CLI: `lf task status` resolves the live successor; `lf task stack` by worktree
resolves the live successor's PR; reteam rebind targets the current successor.
