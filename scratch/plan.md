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
only when `sessions`, `planning` or `graphs` moved since its last successful
read, or when its own next availability deadline has passed.

**Server (`TitleReader`).** The reader becomes a registry like
`lifetime::Watcher`, holding only terminals with a live viewer. `Hosted`
already computes `viewed` each pass (`hosted.rs:223`); on 0 → 1 it registers
the terminal with a clone of the host's `waker` and asks for one immediate
read, on 1 → 0 it unregisters. The thread blocks on its channel. While at
least one terminal is registered it follows `read_stream::watch_revisions`
and is sent a look when a title domain moves; with none registered it holds
no subscription. On a look, or when the earliest deadline among its terminals
arrives, it reads the titles, at most one sweep per 500 ms with the last change
always read. A name that differs from the last one sent goes to the terminal's
channel and the reader writes the waker. `OutputTitle::tick` only drains the
channel; `OutputTitle::deadline` and its line in `Hosted::deadline` go. The
`held-terminals` thread still never touches SQLite. A viewer that attaches
gets the stored `Output` title at once and the fresh one a moment later.

Each sweep borrows one store from `SqliteStore::open_read_only`; the pool
(`loopflow-storage/src/pool.rs`) keeps the connection and its 256-entry
statement cache across borrows and evicts it when the database file is
replaced. Idle, the reader borrows nothing.

**Shared wake.** `read_stream::watch_planning` (`read_stream.rs:926`, one
caller in `server/subscriptions.rs:189`) becomes
`watch_revisions(database) -> watch::Receiver<Option<StoreRevisions>>`; the
observer's `planning` list becomes that list. Subscriptions keep filtering
`planning`; the reader filters its three domains. No title knowledge in
`read_stream`.

**Deadline.** `session_title` returns, beside the names, when the title can
next change with no write: the first entry of the reading's `deadlines`.
`standing` gets it from the same `Reading` it already acquires. Every
successful read overwrites the terminal's deadline, even when the name is
unchanged: streaming text moves `observed_at` without moving `sessions`, so a
deadline that fires early finds a later one.

**Failure.** The reader advances its seen revisions only after a sweep
succeeds. On an error it logs at debug and retries on a backoff (1 s doubling
to 30 s) without holding a connection, so a rename that hit `SQLITE_BUSY` on a
quiet Machine still lands.

**Viewer (`TerminalTitle::refresh`).** Same gate inline: on its existing
500 ms check it reads `store.revisions()` (cached statements) and reads names
only when a title domain moved or its deadline passed. No extra thread in
viewer processes.

**Cached statements.** `now` becomes a bound parameter of `available_sql` and
`summary_query`, in every caller (`projected_session_summary`,
`projected_session_summaries` at `sessions.rs:708`, `inventory_query`). The
three `session_projection_inputs_in` statements, `summary_query`, the
single-row reads on the title path (`terminal_session`, `session`,
`attachment_in`, `task`, `task_graph_row`, `latest_in`, the short Task id) and
`SELECT revision FROM history.history_revision` in `revision_map`
(`sqlite/revisions.rs:115`, read on every observer and viewer check) use
`prepare_cached`.

**Host loop.** With the title deadline gone, an idle `held-terminals` loop
still returns at the 10 s fallback in `Hosted::deadline`; tests count title
reads, not poll returns.

### Choices made for review

- **Only viewed terminals get titles** (concept review Q2, its recommendation).
  A terminal with no tab has nothing to rename; on a working Machine the 25
  parked conversations cost nothing. Passed over: refreshing every hosted
  terminal so the retained title is current before anyone attaches; the
  attach-time read covers that in milliseconds.
- **Keep the per-Session summary read, cached** (review Q3, against its
  recommendation of a batched four-field title query). With only viewed
  terminals swept, N is the open tabs, usually a few. A second query beside
  `session_summary` is a second place the availability rule could drift.
  Pass 1 measures; the slim batched query is the next step if a sweep of the
  viewed set costs over 1% of a core, before stretching the 500 ms.
- **Viewers gate inline on revisions** (review Q1b, against its 1a). Sending
  `Event::Title` over the view stream would retire viewer SQLite and make
  remote cmux/tmux renames work, but it is a wire protocol bump (4 → 5) and new
  remote behavior; listed under Deferred findings.
- **Pooled borrow per sweep** over a pinned private store (review): the pool
  already keeps statements warm and handles a replaced database file.
- **500 ms stays the worst-case delay** after a change.
- **No liveness clock.** A direct child's exit settles through `Hosted`; a
  silent external death is swept by `recovery-wake` within 60 s, and that
  write moves `sessions`.

## Acceptance check

```
cargo test -p loopflow-execution --lib terminal_title::
```

passes with tests that: start a reader over a store with a Session on a held
terminal, receive the first name, then see the reader's read count stay fixed
across several throttle periods with no write; rename the Session and receive
the new name; put the Task at an interactive step and receive `· Design`;
record a yield and receive `· Available`; seed activity 119 s old and receive
`· Available` with no write; bump `observed_at` without a revision move and see
the reader re-arm to the later deadline; fail one sweep and see the rename land
on retry; see no reads for a terminal with no viewer and one read when a viewer
attaches.

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
- Store unreadable: debug log, retry on backoff with seen revisions unchanged.
  The terminal keeps its last title. Nothing on the byte path waits.
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
- `watch_planning` and the observer's planning-only list.
- Title registration for terminals with no viewer.
- `.chain(Some(self.title.deadline()))` in `Hosted::deadline`.
- `now` formatted into `available_sql` / `summary_query` text.
- Must survive: titles sent through the terminal's serialized output stream; a
  departing attachment stops publishing (`output_title`'s attachment check and
  `replaced_attachment_stops_observing_session_names`); viewers keep naming
  their own host windows.

## Passes

1. **Gate and measure.** `watch_revisions`, the viewed-only reader registry,
   deadline re-arming, retry backoff, host wake-up, `now` bound and the title
   and revision paths on `prepare_cached`, the tests above. Shows: read count
   flat while idle; per-read cost with warm statements (time 1,000
   `output_title` calls in a test and note it here); whether a sweep of the
   viewed set twice a second fits under 1% of a core. If it does not, the
   batched title query comes next.
2. **Viewer and benchmark.** `TerminalTitle::refresh` on the same gate. Add a
   held-terminal phase to `scripts/benchmarks/server_idle.py` (30 Sessions
   seeded with held terminals, hosted by the server; idle, then with one
   writer) with a CPU budget in its
   budgets file; if hosting held terminals headless from the script is not
   possible, the phase is left out and the reason noted in the PR.

Done when an idle server with held terminals shows no `title-reader` SQLite
work, and a rename, a waiting step and availability each change the tab title
without a keystroke.

## Concept review (pre-build, a82a7cc0)

- Batched four-field title query: not adopted; measured in pass 1 (choices).
- Viewed-only titles, pooled borrow per sweep, `watch_revisions`, deadline
  re-arm on every read, retry after a failed sweep, subscribe only while a
  terminal is watched, cached `history_revision`, the 10 s host fallback: in
  Design and pass 1.
- A terminal linked after `Hosted::new`: `bind_session_terminal` moves
  `sessions`; the first read finding nothing is correct.
- `Event::Title` on the view stream: deferred.

## Deferred findings

- **Server-pushed titles for viewers.** `Event::Title { terminal, workspace }`
  on the view stream like `Event::Directory`; viewers stop reading SQLite and
  remote (`lf --machine`) cmux/tmux tabs and workspaces rename. Takes a held
  terminal protocol bump 4 → 5 (open views re-exec once) and moving the
  cmux/tmux rename onto the event. Direct chats keep `TerminalTitle::refresh`.
- **`recovery-wake` on the shared observer** (`server/recovery.rs`): a second
  `StoreChanges` thread on `loopflow.db`; fold onto `watch_revisions`.
- **`lifetime::Watcher`'s 2 s poll** of `session_attachment` for exec'd Codex
  clients: drive from `sessions` moves on `watch_revisions`.
- **Uncached `prepare`/`query_row` off the title path**: repository-wide
  storage cleanup.

Security review: not needed; no auth, outbound data or untrusted input changes.
