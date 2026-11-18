# LOO-959: Task conversations and Graph runs come back after a kill, a reboot or a server restart

Stacked on LOO-953 (`jack/loo-953-remove-process-rows-liveness-on-sessions-and`).
The PR targets that branch until it merges. Direction and the four questions
come from `scratch/direction.md`. LOO-953's design is in `scratch/base-loo-953-design.md`.
Paths below are under `rust/` unless they say otherwise. Everything was measured
or read on 2026-11-17.

## Outcome

After a kill, a reboot or a server restart, every open Task on this Machine
either runs again or says what stopped it and which one command continues it.

- A Task conversation that ended without the person quitting comes back on its
  own. It restarts from its saved thread, in a held terminal, within seconds of
  the server noticing. An open Desktop pane reconnects to it.
- A Task conversation the person quit (exit 0 or 130, or an lf command they
  ran to stop it) stays quit.
- A Task run whose carrier died (the `lf task run` or `lf ship` process that
  keeps the Task moving) is shown as died, on the step it reached, and carried
  again from its checkpoint on the same backoff ship restarts use today. After
  5 deaths in a row it stops and offers `lf task run <ID>`.
- After a reboot, the server starts what was running, one start at a time,
  most recently active Task first.

## Demo

```bash
# kill
lf task status LOO-x            # Conversation: running
kill -9 <provider pid>          # the Task conversation's claude
lf task status LOO-x            # Conversation: died 12:01 (exit 137) · starting again
# quit
# type /exit in the pane
lf task status LOO-x            # Conversation: quit 12:03; `lf resume --task LOO-x` opens it
# reboot with three Tasks running (one mid-Graph, unapproved)
lf task status LOO-a            # Conversation: running
lf task status LOO-b            # Run died at implement; the server's next pass carries it again
```

## The four questions, answered

### 1. Limits: no cap; the start reports what ran out

Measured on this Machine (Mac, 128 GiB, 16 cores). At the time it held 105
`claude` processes, 135 `codex` processes and 73 live held terminals:

| Resource | Per managed conversation | Machine limit | Room left |
| --- | --- | --- | --- |
| PTY | 1 (held terminal) | `kern.tty.ptmx_max` = **511**, shared with every Terminal/Desktop tab; 144 in use | ~360 |
| Memory | claude 187 MiB RSS on average | 128 GiB | ~600 |
| File descriptors | ~28 (claude) | `kern.maxfiles` 50000; the server raises its own limit | thousands |
| Processes | ~2.4 (provider plus MCP children) | `kern.maxprocperuid` 10666; 1243 in use | thousands |
| Server connections | 0 (viewers attach to `held.sock`, not `machine.sock`) | 64 | — |

PTYs run out first, at 511 for the whole Machine, and well before hundreds of
conversations. Headless Graph steps use no PTY. The SQLite write lock is not
per-conversation: a start costs a few short transactions.

**Decision (made for review): no limit, and no eviction.** A start that fails
(openpty `EAGAIN`, provider missing, no agent selected) is recorded on the
Session as a launch failure with its reason, for example "no PTY left: 511 in
use (kern.tty.ptmx_max)". `lf task status` shows it, and the pass retries it on
the backoff below. `lf machine doctor` reports PTY headroom.

Passed over:
- A configured cap on managed conversations. It refuses a context; the OS
  already refuses at the real limit and says why.
- Evicting idle conversations so they start again when steered (LOO-958's
  wake). That is worth doing once PTY headroom is a measured problem, not
  before. It is recorded in `questions.md`.

### 2. Natural recovery: what notices each failure today

From reading the code. The full table is in `questions.md` under "failure table".

| Failure | Noticed by | Does | Gap (this Task) |
| --- | --- | --- | --- |
| Provider killed, holder alive | Holder `child.wait`, then server `native_exit::settle` (`harness/native_exit.rs:53`) | Records exit 137 and ends the terminal | **Nothing restarts it.** Idle wake skips a dead pid (`steer.rs:132`), so steers wait forever |
| Holder dies | `held.rs:245` `unreachable`; `Registry::adopt` at start (`transport/registry.rs:565`) | Settles Unknown (no exit) | Same: nothing restarts it |
| Person quits | Same settle path | Exit 0 or 130 | Told apart only by the exit code; an lf-issued stop is not recorded at all |
| Graph step child killed | `GraphRunner::spawn` waits (`lf/commands/graph.rs:912`) | Graph fails with 1; the carrier retries from the checkpoint (3 attempts, `graph.rs:147`) | None |
| Graph process killed | Carrier `spawn_task_child` (`graph.rs:419`) | Any signal becomes 130 "interrupted" and the run ends (`exit_of`, `graph.rs:374`) | A SIGKILL is treated like a person's ctrl-C |
| Carrier (`lf task run`) killed | Nothing | — | The Task reads `moving` (`durable.rs:410`, SQL with no OS check) until a read reaps it. Nothing continues the run |
| Server restart or upgrade | `Registry::open`/`adopt` rehosts live holders | Holders survive; the first wake reading delivers steers | None for conversations. No recovery pass runs in the server |
| Reboot | `adopt` settles every terminal as Unknown | — | Nothing restarts conversations or Graphs |

The only recovery that exists is **lazy**: `ops/recovery.rs` `recover()` runs
before some reads (`bin/lf.rs:2698-2718`). It reaps agent rows, restarts
shipping Tasks with a dead carrier (`ops/task/ship.rs:588`, backoff of 1 to 30
minutes) and restarts limit waits. A Machine nobody reads recovers nothing.

### 3. Graceful restarts: write ahead, settle on the way up

The server has no SIGTERM handler and writes nothing on the way down. That is
fine, and it stays that way. Every fact recovery needs is written before it is
needed:
- `meta.json` before each start;
- the holder's `exit.json` at each child exit;
- `held_terminals`, Session and Graph-run rows, which LOO-953 gives their own
  pid and OS start time.

On the way up (launchd `RunAtLoad`+`KeepAlive`; systemd `Restart=on-failure`):
1. `Registry::open` rehosts live holders and settles the rest. This exists
   today.
2. **New:** the server's recovery pass reads the store and works through what
   should be running (below). The first reading after start is the
   reboot/restart case.

There is no ordering problem between the two. A Session whose holder is still
pending (it holds its lock but has not answered) is not dead, so the pass
leaves it alone.

Cleanup that must happen after an unorderly stop already does: stale
`machine.sock`/`held.sock` are unlinked at the next bind, and terminal folders
are removed once their end is settled. This Task adds the store's liveness and
retention to the same pass (question 4).

### 4. Ongoing cleanup: inventory

Full table, with writers and removers, in `questions.md` under "cleanup
inventory". **Nothing removes:**
- In the store: `agent_sessions`, `session_events` (only usage receipts are
  merged), `steers`, `held_terminals`, `desktop_shells`, Graph run and step
  rows, `creation_receipts` (intended).
- Files under `$LF_HOME`: `logs/lf.log` (no rotation), `logs/terminal-<id>.log`,
  `logs/graph-<id>.log`, `logs/server.log`, `runs/<xx>/<key>/` capture
  folders, `screens/*.ghostsnp`, `holders/<build>/` folders of retired builds,
  `workspace-locks/*.lock`.
- In checkouts: `.lf/prompts/*` and `.lf/journal/traces/*` (they go only with
  the worktree).
- Other: owners left by earlier `session serve` builds; Codex, Pi and OpenCode
  transcripts.

Store retention (`close_dead_processes`, `forget_processes`, receipt pruning)
ran only through `lf process prune`, by hand; the server pass now runs it hourly
(`ops::recovery::retain`, slice 3) and the command is gone. Worktree retirement
runs every minute, from `lf install schedule`'s `lf task retire` job.

Also seen: **31 `lf server run` processes from `target/debug` builds of other
checkouts**, up to 20 hours old (fixtures of LOO-932/935/916). Nothing reaps a
fixture's server. That is fixture hygiene in those Tasks, not this one.

In scope here: the server pass takes over what `recover()` and LOO-953's
liveness pass do, so a Machine nobody reads still settles and recovers. The
file and row growth above is out of scope. It goes to a separate retention
Task, noted in `questions.md`.

## Design

### Source of truth

No new registry. The pass reads three things:
- **Task conversation:** `task_primary_sessions` joined to `agent_sessions`,
  with liveness from LOO-953's Session identity columns (pid and OS start time,
  read by `ops::recovery::due_task_conversations`).
- **Task run:** the carrier that makes the Task `moving` or `shipping`:
  `task_graphs.lf_process_id` (with LOO-953's `run_pid`/`run_os_started_at`)
  or the ship event's carrier. The Graph run under it (`graph_processes`, with
  its own `pid`/`ended_at` from LOO-953 slice 2a) names the step reached.
- **Task:** open and claimed on this Machine (`task_claimed_here`), and started
  (it has taken up a Graph).

### Killed vs quit

A Session's agent ended **by the person** when its launcher's exit code is 0
or 130 (`/exit`, ctrl-D, ctrl-C at the prompt). Everything else counts as
**died**: exit 137 or any other non-zero code, an end with no exit (the holder
died, a reboot), or an unfinished agent row whose recorded process is gone.

Slice 1 found no lf command a person runs that ends a held Task
conversation's agent: the server has no stop operation, Desktop has no close
that ends one, and `lf task interrupt` stops Graphs only. A move completes the
Session, which the pass skips. So `agent_stopped_at` has no writer and was not
added. The exit code needs no new writer, so a released lf that settles an end
during upgrade skew still records a quit correctly. If a stop command is ever
added, it writes a per-agent-generation marker then.

Open counterexample (acceptance check 5, not yet run): at logout or shutdown,
launchd sends SIGTERM to every process. A provider that catches SIGTERM and
exits 0 before its holder goes would read as a quit, and the conversation
would stay down after a reboot. Send SIGTERM to a stand-in holder's process
group and read what `exit.json` records. If an exit 0 can land, the holder
records that it was itself told to stop (`exit.json` `holder_stopping`), and an
exit after that counts as died.

### The recovery pass

A second watcher thread in the server, shaped like `server/wake.rs`. It
re-reads on a store change signal and at least every 60 seconds. There is no
clock per Session. Each reading:

1. **Settle.** Close owner rows whose pid is dead (LOO-953 approach 4), reap
   orphan agents (`reap_agent_processes`), and run `restart_waiting_runs` and
   the rental check. These are today's `recover()` work, moved here. A Graph
   run closed here with no exit reads `Unknown` from `Run::result()`, shown as
   "died".
2. **Task conversations that should run.** A Task's primary Session should run
   when all of these hold:
   - the Task is open, claimed here and checked out here (today's starter
     also runs at `lf task checkout`, before any Graph);
   - the Session's checkout is the Task's;
   - no agent is live and no holder for it is pending;
   - its last end was not by the person.

   Such a Session is started with `session.continue`'s own path:
   `continuing()` (`server/service.rs:907`) followed by `Registry::start`. The
   request id is `recover:<session>:<ended agent>:<attempt>`. The store
   counts the start first and the backoff holds the next one, and the
   registry answers a repeated request with its terminal, so one death never
   starts two providers, across a server restart too.
3. **Task runs whose carrier died.** `restart_ships` becomes `restart_runs`,
   one loop over this Machine's open, claimed Tasks whose carrier is dead:
   - shipping: `lf --task <ID> ship` again, as today;
   - moving (a `task_graphs` carrier, not shipping): `lf task run <ID>`
     again, which continues from the checkpoint. The dead carrier is cleared
     in the same write that records the death.

   Both use `restart_lf_in`, the limit-wait check and the backoff below. A
   carrier that exited on its own wrote its outcome (an interrupt included);
   one still named with no outcome died.

   Continuing never approves Auto-Ship a person did not already approve.
   Approval is written in two places only: at run registration when no
   interactive step is left (`task_work.rs:307`, so a Task launched that way
   is already shipping and `lf ship` carries it), and when a run arrives at
   `end` (`Approval::Route`, which a person's `lf ship --stop` blocks). A
   continuation of a run launched with an interactive step ahead still has
   that step ahead, and stops at it. One case moves approval earlier: an
   `lf -b task run` that ran an interactive step headless, then died past it.
   Continuing it registers with no interactive step left and approves at
   launch instead of at `end`; `-b` already hands the Task off, so the
   intent is the same.

**Pacing.** At most one start in flight. The next start begins when the
previous one's held terminal is running, or after 3 seconds. Starts go most
recently active Task first (`tasks.updated_at`). Today's 55 open Tasks with
primary Sessions come back in about 3 minutes, and no transaction is longer
than one Session's start.

**Backoff.** One schedule for Sessions and carriers: `restart_due`
(`ship.rs:120`), 1 minute doubling to 30. A Session that dies again before
finishing a turn waits; after 5 deaths with no finished turn the pass stops
and the Task shows "died 5 times since 09:14 (last: exit 1); `lf resume --task
LOO-x`". The count is `agent_sessions.starts_since_turn` plus `last_started_at`;
a finished turn resets it. A carrier's restarts, shipping or moving, are one
count on `task_graphs` (`restarts`), reset by a finished step
(`ship_progressed`'s rule). After 5, a moving Task shows "died 5 times at
implement; `lf task run LOO-x`"; a ship keeps restarting on the longest
backoff, as before.

**Never starts:**
- bare Sessions: chats and batches (`lf <skill>` at a terminal, `lf -b`);
- Graph-step Sessions (the Graph owns them);
- Wave and repository conversations (decision below);
- a second provider on one thread;
- a second carrier for a Task (`select_task_run` already checks with OS
  evidence, and `restart_ships` already skips a Task a live `lf task run`
  carries).

### One start path for Task conversations

`start_conversation_beside` (`ops/task.rs:2293`, run by `lf task run`,
`lf ship` and `lf task checkout`) calls `start_task_conversation` →
`start_in_background` (`ops/human_session/primary.rs:104-119`), which prepares
the primary Session (`ensure_task`) and starts it unless it was opened
before. The pass's rule covers the start. So those commands keep only the
prepare half (`ensure_task`), whose store write wakes the pass, which starts
the Session. `start_in_background`, `start_task_conversation`, the
`agent_input_timing` phases they record and the start half of
`start_conversation_beside` go. The no-agent and no-CLI skips move into the
pass, where they become a recorded launch failure instead of an `info` log
line.

### What people see

The facts go on the records they describe, not into a new Task-level state.

- **Session.** `SessionState` (`unknown · active · detached · closed`,
  `ops/human_session.rs:241`) stays the one answer to "is it running". A
  closed Session gains `ended: {at, by: person | died, exit_code}` and
  `start: {attempts, last_started_at, next_at, gave_up, failure}`, null when
  nothing is pending. `failure` carries a launch failure's reason ("no PTY left: 511 in
  use (kern.tty.ptmx_max)"). A Session being started again reads `closed`
  with `start.next_at` due, until its agent is live.
- **Task run.** The Task's run line, already derived by
  `execution_of_latest_graph`, shows a dead carrier as `died at <step> {at}`
  with `restarts` and `next_at`, or the `lf task run` command after giving up.
- **Task status** names its primary Session (`primary_session_id`) beside
  `overview.sessions`, so the conversation line is that Session's state, read
  the same way `lf session list` reads it.

`lf task status` prints "conversation: died 12:01 (exit 137) · starting
again" from those fields; "conversation" is the product's word for the Task's
primary Session, and the code keeps calling it a Session. Wire fields go into
`tests/fixtures/dto/session.json`, `task_status.json` and each language's
fixture test, with no defaults. Desktop decodes them and shows the line. No
new view.

### Decisions made for review (headless; passed-over options recorded)

- **Only Task conversations come back on their own.** Wave and repository
  conversations, and bare Sessions, are only reported. Passed over: every
  managed conversation. That fits "the task conversation should ~always be
  awake" (LOO-958), but nobody has said it for Waves.
- **A dead carrier is always carried again, by generalizing `restart_ships`
  to moving Tasks.** Passed over: a separate Graph-run continuation keyed on
  `graph_processes`, continued only when it cannot approve Auto-Ship and
  otherwise offered, with a `continues` column. Approval is recorded at
  registration and at `end` (see the pass, step 3), so the predicate was
  always true except for `-b`, where it matches the intent; the Graph process
  under a live carrier is already retried by the carrier. Never continuing
  was passed over too: a conversation restart kills an attached carrier, the
  commonest death.
- **No conversation cap and no eviction** (question 1).
- **The pass lives in the server, and the lazy `recover()` on reads goes.**
  Passed over: keep both. That would mean two schedules for the same work.
  Without a server no held terminal can start anyway (`require_server`,
  `transport.rs:393`).

## Acceptance check at gate

A headless fixture with a `MachineServer` and a stand-in provider:
1. Start a Task, then SIGKILL its conversation's provider. Within one pass, a
   new held terminal runs `lf session resume` of the same Session, and
   `lf task status --json` gives the primary Session `active` with
   `start.attempts` 1 (the earlier end is not on the Session while the new
   agent runs; see slice 4).
2. The stand-in exits 0. After a pass, no new terminal starts; the Session is
   `closed` with `ended.by` `person` and `recovery` null.
3. Stop the server, kill the holder, then start the server. The conversation
   starts once; repeating the pass starts nothing.
4. Record a moving Task whose `task_graphs` carrier pid is dead, at step
   `implement` with an interactive step ahead. One pass launches one
   `lf task run`, which stops at the interactive step; the Task is not
   shipping. A second pass launches none. With the carrier dying 5 times in a
   row, status gives `died 5 times at implement` and `lf task run`, and no
   pass launches again.
5. A SIGTERM to a stand-in holder's process group (shutdown) leaves the
   conversation counted as died, and it starts again with the next server.

## Exclusions

- Wave, repository and bare conversation recovery, and eviction.
- Retention of files and rows (the "nothing removes" list).
- LOO-958's steer-wakes-a-stopped-Session.
- Rented Machines, beyond moving `rental::recovery::check` into the pass
  unchanged.

## What goes

- `ops/recovery.rs` lazy recovery and its read hook (`bin/lf.rs:2698-2718`).
  The work moves to the server pass.
- `start_in_background`, `start_task_conversation` and the start half of
  `start_conversation_beside`.
- `restart_ships` as a ship-only loop: it becomes `restart_runs`.
- `lf process prune` / `lf monitor prune`: gone (slice 3); the pass does their work.
- `exit_of` mapping every signal to 130. It becomes SIGINT/SIGTERM/SIGHUP
  interrupted, and anything else a failure the carrier retries
  (`graph.rs:374`, the predicate at `:254`).

Must survive:
- ship restarts and their backoff;
- limit-wait restarts;
- agent reaping;
- rental checks;
- the 3-attempt Graph retry and the repeated-failure stop;
- the idle wake.

## Slices

1. **A killed Task conversation comes back.** Built:
   - migration `task_conversation_recovery.sql`: `agent_sessions.starts_since_turn`
     and `last_started_at` (no `agent_stopped_at`, see Killed vs quit);
   - `SqliteStore::conversations_to_start` (storage
     `sqlite/session_recovery.rs`): open Task, primary Session, cwd is the
     Task's checkout here, not completed or moved, launcher exit not 0/130,
     fewer than 5 starts. Index-driven; 24 ms on this Machine's store (54 rows,
     47 of them live agents);
   - `ops::recovery::due_task_conversations`: died (ended, or unfinished with a
     dead identity), backoff passed (`repair_backoff` from `last_started_at`; the
     first start after a death is immediate), claimed here;
   - `server/recovery.rs`: the watcher thread, started beside the registry. It
     reads on a planning/sessions/processes revision change or every 60 s,
     skips Sessions with a live or pending terminal, counts the start
     (`record_conversation_start`, guarded on the same ended agent), then starts
     `continuing()` with request `recover:<session>:<agent>:<attempt>`.
     Starts run one at a time; each blocks until the holder reports a pid.
   - Tests: `server::recovery::tests::` (killed starts once per backoff, quit
     stays quit, gone-without-end counts as died, a held terminal is not
     started or counted, abandoned or out of starts is not started).

   Taught: request dedupe lives in the store (attempt count plus backoff), not
   only the registry's in-memory map, so a restart cannot double-start. On this
   Machine the first pass would start 7 conversations that died earlier (one
   137, five exit 1, one with no end recorded since 2026-10-22).

   After merging base `e66b21801c`, `server::recovery::tests::` still pass (5).

   Left from slice 1: acceptance check 5. (`conversation` in
   `lf task status --json` shipped in slice 4.)
2. **Restart and reboot.** Built:
   - a finished turn (the Stop hook's `yielded`, `record_native_turn`) resets
     `starts_since_turn`. A resumed provider records no `yielded` at startup
     (`server/boundary.rs` maps only `Stop` and `UserPromptSubmit`);
   - pacing: after a start the pass waits until the Session's attachment names
     a new agent, or 3 s;
   - `agent_sessions.start_failure` (in `task_conversation_recovery.sql`,
     which no longer adds `agent_stopped_at`): the reason a start failed,
     cleared by the next that succeeds. The pass checks the provider's CLI on
     its `PATH` first ("claude is not installed on this Machine's PATH");
   - `ops::recovery::recover()` became `settle()`, run by the pass at most
     every 10 s; the read hook in `bin/lf.rs` is gone;
   - `start_in_background` and `start_task_conversation` are gone;
     `prepare_conversation_beside` (was `start_conversation_beside`) only runs
     `ensure_task`, and the pass starts a primary Session that never had an
     agent (request `recover:<session>:new:<n>`);
   - tests: `server::recovery::tests::` (7), the steer turn test, and
     `context_launch_tests::a_killed_task_conversation_comes_back_once_and_a_quit_one_stays_quit`
     (acceptance 1–3: SIGKILL the provider, kill the holder across a server
     restart, quit with exit 0), plus the background-start and
     "agent not installed" fixtures, which now go through the server.

   Taught:
   - A death moves the processes revision, but the pass read while the
     registry still listed the ending terminal, skipped it as held, and then
     slept the full minute (2 of 6 fixture runs). It now re-reads after 1 s
     while a due conversation still has a terminal.
   - Counting the first start of a prepared conversation made its first death
     wait the 1-minute backoff. A first start now counts only if it fails; its
     terminal holds later passes off.
   - Starts now run with the server's environment, not the CLI's: fixtures
     whose stand-in provider must start need
     `MachineServer::start_with_path`. The supervised server's `PATH` already
     adds the Desktop directories (`server/mod.rs` `supervised_path`).
   - `GraphTask`'s server gets the fixture's `PATH`, so the ship-restart
     fixtures (`task_ship_restarts_after_its_process_dies`, `…_after_a_failure_wait_instead_of_at_once`)
     restart through the server. All 13 `task_graph_launch_tests::task_ship_` pass.

   Not done: the reboot half of the demo on a real Machine restart (qa).
3. **Task runs.** Built:
   - `restart_ships` became `restart_runs` (`ops/task/ship.rs`): ships as
     before, then each Task checked out here whose `task_graphs` names a
     carrier that died (`died_run`) gets `lf task run <ID>` again;
   - died means the carrier never wrote its own outcome: unfinished with a
     dead identity, or closed with no outcome. A carrier that failed, was
     interrupted or waits on a limit wrote one and stays stopped, so the
     3-attempt retry and the repeated-failure stop survive. A Task with live
     Graph execution (an orphaned Graph whose carrier died) is not carried;
     the Graph arrives on its own;
   - `task_graphs.restarts`/`restarted_at` (in `task_conversation_recovery.sql`),
     counted with a compare-and-set on the dead carrier before the launch,
     reset by `advance_task_graph_run`, `arrive_in` and `set_step_in`. First
     start immediate, then `repair_backoff` from `restarted_at`; after 5 the
     run waits;
   - the died line overrides the Task run's `idle`/`blocked` execution:
     `waiting` "Run died at demo; the server's next pass carries it again" or
     "… in N min (restart k of 5)", `blocked` "Run died 5 times at demo;
     `lf task run INF-123` starts it again";
   - `exit_of`: SIGINT/SIGTERM/SIGHUP are 130; any other signal is exit 1 and
     a failure the carrier retries;
   - plan correction: a carrier that exits on its own does **not** clear
     `task_graphs.lf_process_id` (only arrival does); the outcome on its
     process row is what tells it from a death;
   - tests: `ship::tests::a_died_run_is_carried_again_at_once_then_later_then_waits_for_a_person`,
     `task_graph_launch_tests::a_task_run_whose_carrier_died_is_carried_again_and_gives_up_after_five`
     (acceptance 4), `task_graph_launch_tests::a_killed_graph_is_started_again_by_its_carrier`.

   Taught: a bare `lf task run` launcher lets go and the carrier is a
   detached process, and the Graph and its steps run in their own process
   groups, so killing the launcher's group kills nothing that matters. A
   SIGKILLed Graph under a live carrier is now retried by that carrier.

   Then prune went into the pass:
   - `settle()` runs `settle_agent_processes()` (reap, then release unrecorded
     ancestry; one entry point, no dry run);
   - `ops::recovery::retain()`, at most hourly and on the first reading, after
     the starts: store retention (`close_dead_processes`, `forget_processes`,
     usage compaction, reclaim), then `remove_dead_process_receipts_at`
     (was `prune_process_receipts_at`; the PID pre-filter went, the Dead check
     already covers it);
   - deleted: `lf process prune`, `run_prune`, `ProcessPruneReport`, its
     renderer, the `monitor prune` alias, `read_process_receipts_at`, the two
     `process_ownership_tests` that drove prune (including one that still
     expected `task list` to settle, dead since slice 2), and the hints in
     doctor, install, diagnosis, agent_process and `docs/dev/*`;
   - tests: `ops::recovery::tests::retain_settles_proven_dead_processes_and_keeps_unknown_ones`,
     `harness::agent_process::tests::failed_sampling_reaps_nothing_and_reports_why`
     (non-macOS only; runs in CI).

4. **Surfaces.** Built:
   - `SessionRecord` gains `ended {at, by: person|died, exit_code}` (interactive
     Sessions, while no agent runs) and `start {attempts, last_started_at,
     next_at, gave_up, failure}` (a Task's conversation that died or whose
     start failed; null once quit). Read by `summary_query` from the agent
     row and its launcher's exit (the two `processes` joins LOO-953 replaces)
     and the three `agent_sessions` counters. `next_at` comes from
     `ops::recovery::next_start_at`, the same rule the pass waits on.
   - `lf task status --json`: `conversation` names which of `sessions` is the
     Task's (added to `sessions` when the overview lacks it); `SessionFacts`
     carries `ended` and `start`. The text form prints `Conversation: running`,
     `died 12:01 (exit 137) · starting again`, `quit 12:03; \`lf resume --task
     X\` opens it`, or `… · started 5 times; …`.
   - Desktop: the Task page shows the primary Session's `conversationLine`
     (`swift/LoopflowMac/Views/WorkSurfaceView.swift`).
   - `lf machine doctor` `ptys`: in use against `kern.tty.ptmx_max`
     (`kernel.pty.max` on Linux), warns under 32 left. 178 of 511 here.
   - Steers from the concept review (`scratch/concept-review.html`, Jack "1) a
     2) a 3) b"): columns renamed `starts_since_turn`, `last_started_at`,
     `start_failure`; types `ConversationToStart`, `conversations_to_start`,
     `record_conversation_start`, `record_start_failure`,
     `CONVERSATION_STARTS`; the wire field is `start`. One restart count:
     `task_graphs.restarts` for shipping and moving runs, reset by a finished
     step and by `ship_progressed`; `count_ship_restart`,
     `reset_ship_restarts`, `SHIP_RESTARTS_COUNTED` and the ship log's
     restart count are gone (`restarted` rows a released lf writes are
     ignored).
   - Docs in the LOO-924 names; the Desktop-close and `lf task interrupt`
     quit sentence is gone; the first start after a death is immediate.
   - Tests: `human_session::tests::a_task_conversation_says_how_it_ended_and_when_it_starts_again`,
     `lf::commands::session::tests::`, `dto_fixtures::task_detail_says_each_fact_once`,
     `doctor::tests::pty_headroom_…`, Swift `DTOFixtureTests.sessionEndingFixture`,
     and `context_launch_tests::a_killed_task_conversation_…` now reads
     `lf task status --json` (running with `start.attempts` 1; quit with
     `ended.by` person, exit 0, `start` null).

   Taught: while a restarted conversation runs, `ended` describes the current
   agent (none), so acceptance check 1's "the earlier end `by: died`" is not
   readable; `start.attempts` 1 and `last_started_at` say it came back. The
   earlier agent's end is in provider transitions if anyone needs it.

   **Delivery waits on LOO-953's Session columns** (`agent_sessions.agent_pid`,
   `agent_os_started_at`, `agent_ended_at`, `agent_exit_code`). When they land
   on its branch, sync onto it and rewrite both reads that join `processes`
   for the agent's end: `conversations_to_start` (storage
   `sqlite/session_recovery.rs`) and `summary_query`'s `agent_end` columns
   (`sqlite/sessions.rs`). Not added here (steer acfa763f).

   Left: acceptance check 5 (SIGTERM to a holder's group), the real reboot (qa).

5. **What the count and the lines promise** (concept review after qa,
   `scratch/concept-review.html`; choices 1a/2a/3a made for review,
   `questions.md`). Built:
   - A person's start resets the restart count. qa had seen `running (started
     again 15:39, attempt 3)` after a person's resume. `lf session resume`
     calls `ops::recovery::record_start`, which sets `starts_since_turn` to 0
     and clears `start_failure` (`reset_conversation_starts`) unless
     `LF_RECOVERING_SESSION` (`RECOVERING_ENV`, set on the pass's own start)
     names this Session. It is not in the attach write: the pass and a person
     both start through `lf session resume`, and only the launcher knows who
     asked. A `--prepare` counts as a person's. `docs/sessions.md` says so.
   - One waker for one schedule. `restart_runs` returns the soonest ship or
     died-run backoff it skipped; `settle()` passes it up; `Watcher` keeps
     `conversations_due` and `runs_due`, wakes at the earlier, and a reading
     settles when a run came due even inside the 10 s throttle. Limit waits
     still wake on the minute tick.
   - A promise only when someone keeps it. `conversation_line` takes
     `server_runs` (a connect to `machine.sock`) and reads `· starts again
     when the server runs` without one. The DTO is unchanged.
   - Tests: `server::recovery::tests::a_persons_start_counts_the_passs_starts_from_zero`,
     `…::a_runs_backoff_wakes_the_watcher_and_settles` (Watcher-level, not a
     live carrier), `lf::commands::session::tests::with_no_server_a_died_conversation_waits_for_one`.

   Recorded, not built: resuming a conversation with no transcript (killed
   before its first turn) fails five times (3a); the SIGTERM quit window
   (`holder_stopping`) waits on real Claude's SIGTERM exit code.

## Remaining

Nothing left to implement in this Task's scope. Left for later steps:
- gate: the named checks under Checks, and CI's full matrix;
- a person: acceptance check 5 against real Claude (does a SIGTERM'd provider
  exit 0?), and the real reboot with three Tasks;
- delivery: when LOO-953's Session columns land, rewrite
  `conversations_to_start` and `summary_query`'s `agent_end` onto them (slice 4).

## Checks

- `cargo test -p loopflow --lib server::recovery::tests::`: 7 pass (implement).
- `cargo test -p loopflow --test integration context_launch_tests::a_killed_task_conversation`:
  passes 4 of 4 after the ending-terminal fix (implement). `context_launch_tests::a_task_`:
  4 pass. `cargo test -p loopflow --lib task_graph_launch_tests::task_ship_`: 13 pass.
- `cargo test -p loopflow --lib task_graph_launch_tests::a_task_run_whose_carrier_died`,
  `…::a_killed_graph_is_started_again_by_its_carrier`, `ship::tests::` (4),
  `task_graph_launch_tests::task_ship_` and `server::schema::tests::` (21): pass (implement, slice 3).
  Main `2593c489d7` merged after them (`a1568e4c59`, 194 files, no conflicts).
- On the merged tree with prune deleted (implement): `server::recovery::tests::` (7),
  `ship::tests::`, `ops::recovery::tests::`, `harness::agent_process::tests::`,
  journal and diagnosis tests (62 pass); `binary(process_ownership_tests)`,
  `lf::tests::`, `cli_discovery::` (83 pass); `documented_commands::` (4 pass);
  clippy clean. The non-macOS sampling-failure test runs in CI.
- Gate: `test(performance_tests::)` and `test(store::query_plans::tests::)` (new
  `local_task_runs` read over `task_graphs`), `binary(served_reads)` (server startup changed), `test(session_cli_tests::)`,
  `test(process_ownership_tests::)` and `test(documented_commands::)` (docs edited).
- Slice 4 (implement): `human_session::tests::` (29), `lf::commands::session::tests::`,
  `dto_fixtures::`, `server::schema::tests::` (fixture regenerated), `server::recovery::tests::`,
  `steers_tests::`, `server::wire::`, `id_tests::`, `screens::` (113 pass);
  `sqlite::task_work::`, `query_plans::tests::`, `ship::tests::`, `task_graph_launch_tests::task_ship_`,
  `…::a_task_run_whose_carrier_died`, `…::a_killed_graph_…` (40 pass);
  `doctor::tests::` (6); `context_launch_tests::a_killed_task_conversation` (1);
  `documented_commands::` (4); clippy clean.
- `scripts/test_desktop.sh --filter 'DTOFixtureTests'`: 58 pass (implement).
- Gate: every Swift suite with an inline
  Session fixture (keys `ended`/`start` added to 13 files), `binary(session_lifecycle_tests)`
  (`work_first_entry.json`), `test(performance_tests::)` (`summary_query` gained one PK join).
- qa (private Machine, branch `lf`): kill, backoff, quit, server restart with a live
  holder, simulated reboot (SIGKILL server, holder, provider), died carrier, five-start
  give-up, shutdown SIGTERM (`scratch/qa.html`). Fixed: the pass woke for backoffs only on
  the 60 s tick (`server/recovery.rs` wakes at the soonest `next_at`), and a start under
  way read as "starts again <next backoff>" (`session_ending`). After them:
  `server::recovery::tests::` (7), `human_session::tests::a_task_conversation…`,
  `dto_fixtures::` (52), `lf::commands::session::tests::`,
  `context_launch_tests::a_killed_task_conversation` pass; clippy clean.
- Slice 5 (implement): `server::recovery::tests::` (9), `lf::commands::session::tests::` (2),
  `dto_fixtures::` + `documented_commands::` (56); clippy on storage, execution, loopflow clean.
- qa after the Machine crash (private Machine, branch `lf`): person-vs-pass race
  on `lf resume` (fixed: a person's start is start zero), simulated reboot recorded
  exit 0 for reaped agents (fixed: `Registry::open` settles adoption's ends first),
  shutdown SIGTERM (`scratch/qa.html`). After them: `server::recovery::tests::` (9),
  `steers::tests::a_conversation_on_an_app_server…`, `context_launch_tests::a_killed_task_conversation`.
- A real reboot with three Tasks (a person runs it).

## Docs

Slice 4 checked these against what shipped (`5adee6450c`): `docs/sessions.md`
("After a kill, a crash or a reboot"), `docs/work.md`, `docs/troubleshooting.md`,
`docs/server.md`, `rust/loopflow/src/server/README.md` ("Recovery pass") and
`rust/loopflow-execution/src/transport/README.md` (holder "Lifetime").
