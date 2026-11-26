# LOO-1114: terminal titles follow store changes, not a clock

## Outcome

An `lf server` hosting dozens of held terminals does no title work while the
store is quiet. A Session rename, an attachment change, a Task's waiting step
or a Session becoming available still reaches the terminal title within about
half a second. The SQL on the title path is parsed once per connection.

## What was found

- **Every held terminal wakes the host loop and the reader twice a second.**
  `Hosted::deadline` chains `self.title.deadline()`
  (`rust/loopflow-execution/src/transport/hosted.rs:876`), so the
  `held-terminals` poll returns every 500 ms per terminal even with nothing to
  do, and `OutputTitle::tick` (`terminal_title.rs:466`) queues a request each
  time. The reader then opens a pooled store per request and runs
  `output_title`.
- **One title read is about ten statements, most parsed fresh.**
  `terminal_session`, `session`, `session_attachment`, `task`, then
  `session_summary`: `session_projection_inputs_in` runs three
  `conn.prepare(&format!(…))` (`sqlite/sessions.rs:111,137,145`),
  `SessionProjectionInputs::pin` runs the same three again, then
  `summary_query` (`sessions.rs:468`), then `latest_in`, the short Task id and
  `task_graph_row`. Bare `query_row` also prepares uncached.
- **The summary SQL can never be cached as written.** `available_sql`
  (`sessions.rs:347`) formats `now` into the text, so the statement differs
  every second. Caching it without binding `now` would fill the 256-entry
  statement cache with dead entries.
- **A title depends on more than rows.** `available` is
  `live && <SQL>`; the SQL turns true when `now - observed_at >= 120`
  (`AVAILABILITY_QUIET_SECONDS`) with no write. `SessionProjectionInputs.deadlines`
  already lists those moments. A revision gate alone would leave "Available"
  missing until some unrelated write.
- **Revisions cover every row the title reads.** `agent_sessions`,
  `session_activity`, `held_terminals` → `sessions`; `tasks`, `task_graphs` →
  `planning`; `graph_processes` → `graphs` (`sqlite/revisions.rs`, test
  `task_graph_graph_process_and_activity_tables_move_their_domains`). The
  `session_activity` update trigger ignores a bare `observed_at` stamp.
- **How often they move on a working Machine** (live `~/.lf/loopflow.db`,
  2026-11-26, a handful of agents running, sampled 10 s apart): `sessions`
  +20 then +9, `processes` +2 then 0, `planning` and `graphs` 0. So under work
  a global gate fires about once or twice a second; idle it never fires.
- **A shared wake already exists.** `read_stream.rs` keeps one
  `store-observer` thread per database on `StoreChanges` (kqueue on the WAL,
  30 s fallback) and compares `StoreRevisions`; `watch_planning` is the
  precedent for a non-subscription follower. `lifetime::Watcher`
  (`transport/lifetime.rs`) is the precedent for a held-terminal worker that
  blocks on a channel and wakes the host through its `waker` socket.
- **Process death without a write** is the one input nothing signals. The
  host of a held terminal records its child's exit, which moves `sessions`;
  when the child is gone the terminal ends and has no title. A stale
  "Available" between an agent's death and that write is accepted.
- `ViewerTitle` / `TerminalTitle::refresh` run in each viewer process, only
  under cmux or tmux, on a connection they keep open. Same full read every
  500 ms.

## Design

One gate, used by the server's reader and by viewers: a title is read again
only when `sessions`, `planning` or `graphs` moved since its last read, or when
its own next availability deadline has passed.

**Server (`TitleReader`).** The reader becomes a registry like
`lifetime::Watcher`: `OutputTitle::new` registers the terminal with a clone of
the host's `waker`; dropping it unregisters. The thread blocks on its channel.
It follows the store through the shared `store-observer` (no second
`StoreChanges` thread) and is sent a look when a title domain moves. On a
look, or when the earliest deadline among its terminals arrives, it reads the
titles, at most one sweep per 500 ms with the last change always read. A name
that differs from the last one sent goes to the terminal's channel and the
reader writes the waker. `OutputTitle::tick` only drains the channel;
`OutputTitle::deadline` and its line in `Hosted::deadline` go. The
`held-terminals` thread still never touches SQLite.

The reader keeps one read-only store while reads succeed and reopens after a
failure, so its statement cache stays warm.

**Deadline.** `session_title` returns, beside the names, when the title can
next change with no write: the first entry of the reading's `deadlines`.
`standing` gets it from the same `Reading` it already acquires; nothing new is
judged.

**Viewer (`TerminalTitle::refresh`).** Same gate inline: on its existing
500 ms check it reads `store.revisions()` (one cached statement) and reads
names only when a title domain moved or its deadline passed. No extra thread
in viewer processes.

**Cached statements.** `now` becomes a bound parameter of `available_sql` and
`summary_query`, in every caller (`projected_session_summary`,
`projected_session_summaries` at `sessions.rs:708`, `inventory_query`). The
three `session_projection_inputs_in` statements, `summary_query`, and the
single-row reads on the title path (`terminal_session`, `session`,
`attachment_in`, `task`, `task_graph_row`, `latest_in`, the short Task id)
use `prepare_cached`.

### Choices made for review

- **Sweep every hosted terminal on a move, not only the affected one.** No
  per-Session change marker exists, and a title-specific fingerprint query
  would restate what the shared readers read. With cached statements a sweep
  of 30 terminals twice a second is the working-Machine cost; pass 1 measures
  it. Passed over: per-terminal fingerprint SQL; a title subscription through
  `observe` (would hold process facts and bring back background refresh for
  every hosted terminal).
- **Follow the shared `store-observer`** over a private `StoreChanges` in the
  reader. One WAL watcher per process.
- **500 ms stays the worst-case delay** after a change. Raising it to 1 s is
  the fallback if pass 1 shows the working-Machine sweep above 1% CPU.
- **Viewers gate inline** instead of following the observer: a viewer is one
  terminal in its own process, and two cached revision reads a second cost
  less than a watcher thread each.

## Acceptance check

```
cargo test -p loopflow-execution --lib terminal_title::
```

passes with tests that: start a reader over a store with a Session on a held
terminal, receive the first name, then see the reader's read count stay fixed
across several throttle periods with no write; rename the Session and receive
the new name; put the Task at an interactive step and receive `· Design`;
record a yield and receive `· Available`; seed activity 119 s old and receive
`· Available` with no write.

On the Machine after install: `sample <lf server pid> 2` shows `title-reader`
parked in `recv` with no `sqlite3RunParser` frames, and
`uv run python scripts/benchmarks/server_idle.py` reports the held-terminal
phase under its CPU budget.

## Demo

Open two Task conversations in held terminals, leave them idle, run
`lf server diagnostics` twice a minute apart and `top -pid <server>`: CPU near
zero. Then `lf session rename <id> 'New name'` and watch the tab title change
at once.

## Absent and error states

- Terminal with no Session, or whose attachment has no lf process: no title is
  sent, as now; it is read again on the next move, since a Session may link
  later.
- Store unreadable: debug log, drop the connection, try on the next look. The
  terminal keeps its last title. Nothing on the byte path waits.
- Store file not there yet: the observer already retries; the reader waits.
- Linux (no vnode watch): `StoreChanges` looks every 50 ms; that cost is one
  cached revision read per look in the shared observer, unchanged by this work.

## Exclusions

- cmux and tmux rename commands, workspace-name rules, `display_title`.
- The observer's own cost per WAL write (`LOOK` every 50 ms for 1 s).
- `lifetime::Watcher`'s 2 s retry poll.
- Other uncached `prepare` calls off the title path.

## What goes

- `OutputTitle::deadline`, `OutputTitle.checked`, `TitleRequest`,
  `TITLE_QUEUE`, the per-request `open_read_only` in the reader thread.
- `.chain(Some(self.title.deadline()))` in `Hosted::deadline`.
- `now` formatted into `available_sql` / `summary_query` text.
- Must survive: titles sent through the terminal's serialized output stream; a
  departing attachment stops publishing (`output_title`'s attachment check and
  `replaced_attachment_stops_observing_session_names`); viewers keep naming
  their own host windows.

## Passes

1. **Gate and measure.** Reader registry, observer wake, deadline, host
   wake-up, the tests above, `now` bound and the title path on
   `prepare_cached`. Shows: read count flat while idle; per-read cost with warm
   statements (time 1,000 `output_title` calls in a test and note it here);
   whether a 30-terminal sweep twice a second fits under 1% of a core. If it
   does not, the sweep interval goes to 1 s before anything cleverer.
2. **Viewer and benchmark.** `TerminalTitle::refresh` on the same gate. Add a
   held-terminal phase to `scripts/benchmarks/server_idle.py` (30 Sessions
   seeded with held terminals, hosted by the server) with a CPU budget in its
   budgets file; if hosting held terminals headless from the script is not
   possible, the phase is left out and the reason noted in the PR.

Done when an idle server with held terminals shows no `title-reader` SQLite
work, and a rename, a waiting step and availability each change the tab title
without a keystroke.

Security review: not needed; no auth, outbound data or untrusted input changes.
