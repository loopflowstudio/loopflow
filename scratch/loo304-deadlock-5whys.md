# LOO-304 launch deadlock — 2026-10-04

Jack requested this investigation after the Desktop optimization launch froze
the shared Home. Recovery is confirmed; prevention is not implemented here.
The stuck process has exited, the WAL writer lock is free, a two-second
`BEGIN IMMEDIATE`/rollback probe succeeded without data changes, and
`lf task status LOO-304 --json` returned successfully. This does not establish
that LOO-304 resumed or that the underlying defect was repaired.

## Evidence and causal chain

The launch was the direct, headless implementation step of Flow
`485e492b-134a-4ab6-868d-e98ada7a4468`, PID 22251, parent 20595, in
`/Users/jack/src/loopflow.make-the-current-desktop-workspace`.
It started at 00:35:22 PDT. The captured macOS sample at 00:43:20 PDT
contains 879 samples of the same blocked stacks. Its source is
`/tmp/loo304-lock-stack.txt`; retained copy: `loo304-lock-stack.txt` beside this note.
An OS advisory-lock query independently identified PID 22251 as the SQLite
WAL writer-lock owner. Other commands reported `database is locked`.

1. **Why did one launch stall other Tasks?** It retained the shared database's
   write transaction. `SqliteStore::with_session_driver` takes the connection
   mutex and an immediate transaction, checks driver identity, then runs an
   arbitrary closure before committing (`store/sqlite/execs.rs:392`). This
   makes unrelated writers wait behind provider transport.
2. **Why did the transaction not finish?** The closure awaited a WebSocket
   send through `Handle::block_on`, inside `spawn_blocking`
   (`harness/codex.rs:1213`). The sampled worker is parked inside that exact
   `with_session_driver` / `block_on` chain.
3. **Why could the async operation not progress?** The current-thread runtime's
   main thread was synchronously waiting for the same store mutex:
   `CodexHarness::start_inner → History::record → session_thread → Mutex::lock`.
   Incoming history is recorded inline in the reader task (`codex.rs:1296`);
   history lookup acquires the store mutex (`codex_history.rs:82`,
   `store/sqlite/execs.rs:319`). Production creates a current-thread runtime
   in `engine/agent.rs:1658`. The reader blocks the event loop needed by the
   dispatch worker; the worker retains the mutex needed by the reader.
4. **Why did the two-second timeout not bound the failure?** That timeout is
   itself a Tokio timer on the blocked runtime (`codex.rs:1216`). Moving the
   closure to a blocking worker did not make its socket or timer independent
   of the event loop. A timeout cannot rescue a scheduler that cannot run it.
5. **Why does dispatch hold a database transaction across transport?** The
   documented intent is to serialize native dispatch with driver transfer,
   preventing an old driver from sending after replacement. The transaction
   is being used as a cross-process transport fence. This combines database
   ownership, provider I/O, and history ingestion into one locking boundary.
   The intent is valid; its current implementation creates this cycle and
   gives one Session a Home-wide failure radius.

Source inspection is at commit `6497f1fd9a0538407f7d87c04115ca3ce7645084`;
paths above are under `rust/loopflow/src/`. The captured installed binary was
`lf-ae7eaac17043234d863c026d0529e7eeb6dfa36fc2ec25ff1a3a26b10c88626d`.
Matching stack symbols corroborate the source chain; this is not a claim that
the inspected commit is the installed binary's exact build revision.

## Prevention and proof

Proposed repair: remove synchronous store waits from the provider event-loop
path, preserving ordered history processing and driver fencing. Moving only
the history operation off-thread requires a lock-order audit: holding its
history mutex across a store wait could still block the writer's history
correlation. Do not merely switch runtime flavor, increase timeouts, retry
launches, or remove the authority check before asynchronous dispatch.

The deeper design improvement is to stop holding a Home-wide SQLite write
transaction across provider I/O while preserving cross-process exclusion of
stale drivers. The exact replacement needs design; an unlocked check followed
by send would introduce a driver-transfer race.

Add a deterministic, headless regression using the real store and a local
provider socket on a current-thread runtime. Arrange an incoming history
notification while fenced dispatch is pending. Assert progress, retained
history, and rejection of stale-driver dispatch. Bound the whole fixture with
an external process watchdog, so the regression cannot freeze its own timeout.
Include a stalled socket and concurrent unrelated database write to check the
failure radius. Existing history tests exercise ordering sequentially; the
live smoke test is ignored by default. Those inspected tests do not establish
coverage of this contention schedule. No new tests ran for this prose-only
analysis.

## Limits and related paths

- The exact notification and transport readiness condition that won the race
  are unknown. The sampled cycle and WAL owner establish the deadlock without
  establishing that every launch will reproduce it.
- Host load and low disk space were present earlier, but neither explains this
  circular wait. They are not established root causes.
- `harness/codex_connection.rs:166` repeats the transaction/async-send pattern;
  audit it with the repair. It was not the sampled path.
- `harness/opencode_history.rs:278` also holds this transaction over transport,
  but uses a blocking HTTP client. It shares the broad lock duration concern;
  the same Tokio timer deadlock is not established there.
- The record here does not establish which interruption ultimately ended
  PID 22251. Do not attribute recovery to a specific command without its receipt.

Next action: repair the dispatch/history locking boundary before relying on
another autonomous LOO-304 launch, then run the deterministic regression and
resume its retained Flow. Keep its Desktop optimization edits intact.
