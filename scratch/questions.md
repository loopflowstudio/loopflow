# Open Questions

## Repair chain doesn't recurse

`execute_run_inner` in `triggers/common.rs` creates a repair run and executes it inline (line ~134). When that repair fails, it logs the error and returns — it doesn't re-enter `execute_run_inner`'s post-failure logic (chain depth check → next repair or escalation).

This means only one repair ever fires. The second and third attempts, and the escalation at depth 3, are unreachable in the current execution path.

The `count_repair_chain` tests pass because they test counting in isolation — they don't exercise the dispatch flow.

**Fix options:**
1. Recurse: after the repair run's `execute()` returns, re-enter the same post-failure logic (loop instead of inline execution).
2. Spawn: use `spawn_run_task_with_slot` for repair runs so they go through the full path.

Option 1 is simpler — turn lines 134-155 into a loop that checks the repair's status and retries.
