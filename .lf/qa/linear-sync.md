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

## Rate limits and the drain

A pause lives in `~/.lf/locks/linear-cooldown/<key>` (Home-wide, a bare
epoch-ms deadline; empty means none) and `<key>.op/<Operation>` (one
operation). Every `check` creates the files it reads, so empty ones are
normal. While an installed `lf` holds a Home-wide pause, the branch binary
is held by the same file: copy it aside and remove it to let the branch pass
run, knowing installed processes start sending at once too. The request log
shows which binary sent what only by `lf_process_id` and `command`; the
server's rows have `command: null`.

Any saving branch command (`lf task create '<words>'`) runs one after-save
pass: the saved owner first, then 20 requests oldest-first. `lf task sync
<issue>` drains that one owner now and prints what is still open afterwards
("nothing open" after it just sent everything). `lf task create` prints
"Saved locally; pending Linear sync" whenever Linear is connected, because the
export runs after the answer; read `tasks.issue_identifier` to see when it
arrived. `lf task abandon <id>` cancels a probe's Linear issue.
