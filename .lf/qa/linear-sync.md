Read when the change touches Linear sync or planning export.

# Linear sync

The shipped binary has no Linear stand-in endpoint, so Linear sync is checked
on the Machine's own store and Linear. First compare `schema_migrations` in
`~/.lf/loopflow.db` with `rust/loopflow/src/store/migrations/`: only with
identical sets can the branch `lf` open the real store without making the
installed one refuse it. Find held Tasks with a read-only query
(`task_changes.error`, `tasks.completion_error`, `tracker_writeback` in
`lf task show <issue> --json`). Run the installed `lf task sync <issue>` once to
see the failure, then the branch binary, from `~/src/loopflow` with `LF_*`
unset and stdin closed. Each sync sends that Task's queued state and field
edits to Linear: pick recent ones, and leave the rest to the installed pass.
`~/.lf/logs/linear-requests.jsonl` shows each request by `lf_process_id`, and
counting `target` over a day shows reads that never reach a mutation. Linear
can answer a read just after a write with its previous `updatedAt`.
