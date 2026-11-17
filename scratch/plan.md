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
- A Graph run whose process died is shown as died, on the step it reached. It
  is continued once if continuing cannot approve Auto-Ship. Otherwise it is
  offered as `lf task run <ID>`.
- After a reboot, the server starts what was running, one start at a time,
  most recently active Task first.

## Demo

```bash
# kill
lf task status LOO-x            # conversation: running
kill -9 <provider pid>          # the Task conversation's claude
lf task status LOO-x            # conversation: died 12:01 (killed, exit 137) · started again 12:01
# quit
# type /exit in the pane
lf task status LOO-x            # conversation: quit 12:03; `lf resume --task LOO-x` opens it
# reboot with three Tasks running (one mid-Graph, unapproved)
lf task status LOO-a            # conversation: running (restarted 09:14 after the Machine restarted)
lf task status LOO-b            # Graph: died at step implement 08:57; `lf task run LOO-b` continues from implement
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
runs only through `lf process prune`, by hand. Worktree retirement runs every
minute, from `lf install schedule`'s `lf task retire` job.

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
  with liveness from LOO-953's Session columns. Until that slice lands, it
  reads through the one existing seam, `session_summary(..).agent_process_evidence`.
- **Graph run:** `graph_processes` with its own `pid`/`os_started_at`/`ended_at`
  (LOO-953 slice 2a, built).
- **Task:** open and claimed on this Machine (`task_claimed_here`), and started
  (it has taken up a Graph).

### Killed vs quit

A Session's agent ended **by the person** when either:
- its exit code is 0 or 130 (`/exit`, ctrl-D, ctrl-C at the prompt); or
- `agent_sessions.stopped_at` is set. lf writes `stopped_at` before it signals,
  on any command a person runs that ends a Session: Desktop close of a
  conversation's terminal, `lf task interrupt` when it stops the conversation,
  and `lf session stop` if it exists.

Everything else counts as **died**: exit 137 or any other non-zero code, an
end with no exit (the holder died, a reboot), or a provider crash.

The exit-code half needs no new writer, so a released lf that settles an end
during upgrade skew still records a quit correctly. Slice 1 lists every stop
command; grep starts from `os_process.rs:170` (terminate group) and `desktop.rs:1787`.

### The recovery pass

A second watcher thread in the server, shaped like `server/wake.rs`. It
re-reads on a store change signal and at least every 60 seconds. There is no
clock per Session. Each reading:

1. **Settle.** Close owner rows whose pid is dead (LOO-953 approach 4), reap
   orphan agents (`reap_agent_processes`), and run `restart_ships` and
   `restart_waiting_runs`. These are today's `recover()` work, moved here.
2. **Task conversations that should run.** A Task's primary Session should run
   when all of these hold:
   - the Task is open, started and claimed here;
   - the Session's checkout is the Task's;
   - no agent is live and no holder for it is pending;
   - its last end was not by the person.

   Such a Session is started with `session.continue`'s own path:
   `continuing()` (`server/service.rs:907`) followed by `Registry::start`. The
   request id is `recover:<session>:<ended agent generation id>`. The
   registry dedupes on the request id, including after a restart (`readopt`
   re-inserts `meta.request`), so one death never starts two providers.
3. **Graph runs whose process died** (`ended_at` NULL and pid dead) are closed
   with no exit, so `Run::result()` gives `Unknown`, shown as "died". The step
   reached is the open or last step. Then:
   - **continued** once from its checkpoint, by `lf task run <ID>` launched
     detached, only when the Task is already shipping (`restart_ships` covers
     this) or an interactive step lies ahead of the checkpoint. In both cases
     continuing cannot approve Auto-Ship.
   - **offered** otherwise: the Task's status names
     `lf task run <ID>`, which continues from the checkpoint.

   The continuation records which run it continues (`graph_processes.continues`).
   A run that is itself a continuation and dies before finishing a step is
   offered, not continued again.

**Pacing.** At most one start in flight. The next start begins when the
previous one's held terminal is running, or after 3 seconds. Starts go most
recently active Task first (`tasks.updated_at`). Today's 55 open Tasks with
primary Sessions come back in about 3 minutes, and no transaction is longer
than one Session's start.

**Backoff.** A Session that dies again before finishing a turn waits 1, 2, 4 …
up to 30 minutes, the same schedule as `ship.rs:120`. After 5 deaths with no
finished turn, the pass stops and the Task shows "died 5 times since 09:14
(last: exit 1); `lf resume --task LOO-x`". The count is
`agent_sessions.recovery_attempts` plus `recovered_at`; a finished turn resets
it.

**Never starts:**
- bare Sessions (`lf -b`, a person's own terminal);
- Graph-step Sessions (the Graph owns them);
- Wave and repository conversations (decision below);
- a second provider on one thread;
- a second Graph process for a Task (`select_task_run` already checks with OS
  evidence).

### One start path for Task conversations

`start_in_background` (`ops/human_session/primary.rs:119`, called from
`graph.rs:343`, `ship.rs:82` and `bin/lf.rs:2797`) starts a prepared Task
conversation and skips one already opened. The pass's rule covers both cases.
So `lf task run` stops starting the conversation itself, and its store write
wakes the pass, which starts it. `start_in_background` and its three call sites
go. Its no-agent and no-CLI skips move into the pass, where they become a
recorded launch failure instead of an `info` log line.

### What people see

`lf task status` (and its JSON and Desktop's Task header) gains one
`conversation` value: `running`, `quit {at}`, `died {at, exit}`,
`starting {attempt, next_at}`, or `failed {reason, attempts}`, each with the
command that continues it. The Graph line shows a died run as
`died at <step> {at}`, followed by `continued as <run>` or the
`lf task run` command.

It is a wire field: it goes into `tests/fixtures/dto/task_lifecycle.json` and
each language's fixture test, with no defaults. Desktop decodes it and shows
the line. No new view.

### Decisions made for review (headless; passed-over options recorded)

- **Only Task conversations come back on their own.** Wave and repository
  conversations, and bare Sessions, are only reported. Passed over: every
  managed conversation. That fits "the task conversation should ~always be
  awake" (LOO-958), but nobody has said it for Waves.
- **Graph runs continue only when that cannot approve Auto-Ship.** Passed
  over: always continue and record the approval as the original run's (too
  easy to get wrong), and never continue (loses the common case).
- **No conversation cap and no eviction** (question 1).
- **The pass lives in the server, and the lazy `recover()` on reads goes.**
  Passed over: keep both. That would mean two schedules for the same work.
  Without a server no held terminal can start anyway (`require_server`,
  `transport.rs:393`).

## Acceptance check at gate

A headless fixture with a `MachineServer` and a stand-in provider:
1. Start a Task, then SIGKILL its conversation's provider. Within one pass, a
   new held terminal runs `lf session resume` of the same Session, and
   `lf task status --json` gives `conversation.state` `running` with a
   recorded death.
2. The stand-in exits 0. After a pass, no new terminal starts and the state is
   `quit`.
3. Stop the server, kill the holder, then start the server. The conversation
   starts once; repeating the pass starts nothing.
4. Record a Graph run with a dead pid on step `implement`, with no interactive
   step ahead. Status gives `died at implement` and offers `lf task run`;
   nothing is launched. With an interactive step ahead, one continuation is
   launched, and a second pass launches none.

## Exclusions

- Wave, repository and bare conversation recovery, and eviction.
- Retention of files and rows (the "nothing removes" list).
- LOO-958's steer-wakes-a-stopped-Session.
- Rented Machines, beyond moving `rental::recovery::check` into the pass
  unchanged.

## What goes

- `ops/recovery.rs` lazy recovery and its read hook (`bin/lf.rs:2698-2718`).
  The work moves to the server pass.
- `start_in_background` and its three call sites.
- `lf process prune` / `lf monitor prune`, if LOO-953 has not already deleted
  them: their work is now the pass's.
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

1. **A killed Task conversation comes back** (the slice that can prove this
   wrong). Pieces:
   - the migration for `stopped_at`, `recovery_attempts` and `recovered_at`;
   - the killed-vs-quit predicate;
   - the recovery watcher thread with step 2 only, starting through
     `continuing()` with the deterministic request id;
   - `conversation` in `lf task status --json`.

   Shows: acceptance checks 1–3, the kill half of the demo, and whether the
   registry's request dedupe really holds across a server restart. If it does
   not, the dedupe must move into the store.
2. **Restart and reboot.** Pieces:
   - pacing and ordering;
   - backoff and give-up;
   - launch failures recorded with their reason;
   - `recover()` moved into the pass and its read hook deleted;
   - `start_in_background` deleted.

   Shows: the reboot half of the demo on a real Machine restart.
3. **Graph runs.** Pieces:
   - died-run closing;
   - the step reached;
   - continue-or-offer with `continues`;
   - the `exit_of` signal split.

   Shows: acceptance check 4.
4. **Surfaces.** Pieces:
   - the DTO fixture;
   - the Desktop line;
   - PTY headroom in `lf machine doctor`;
   - docs (`docs/sessions.md`, `server/README.md` "Logout/reboot",
     `transport/README.md`).

## Checks

- `cargo nextest` on the new pass's module and on `transport::registry` tests
  (implement; runs locally).
- The acceptance fixture above (implement; runs locally).
- A real reboot with three Tasks (qa; a person runs it).
