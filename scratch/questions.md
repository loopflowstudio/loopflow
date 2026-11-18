# LOO-959 open questions and kickoff findings

## Choices made headless, for review

- Only Task conversations come back on their own. Should Wave and repository
  conversations come back too? Jack (LOO-958, 2026-11-17): "if you steer a task
  the task conversation should ~always be awake anyway". That was said about
  Tasks only.
- A Task run whose carrier died is always carried again (`restart_ships`
  generalized to moving Tasks), on the ship backoff, offered after 5 deaths
  in a row. Concept review: approval is recorded at registration and at
  `end`, so the kickoff's "only if it cannot approve Auto-Ship" rule held in
  every case but a `-b` run past its last interactive step.
- A continued `lf -b task run` is relaunched as plain `lf task run`, so it
  stops for a person at the next interactive step rather than running it
  headless.
- No cap on managed conversations and no eviction. A start that fails records
  why (for PTYs: "no PTY left: 511 in use (kern.tty.ptmx_max)"). Revisit
  eviction of idle conversations, started again when steered, if PTY headroom
  becomes a problem anyone measures.
- The lazy `recover()` on reads is deleted and its work moves to the server
  pass.
- `start_in_background` is deleted, so `lf task run` relies on the pass to
  start the Task conversation. The cost is latency: one store change signal.

## Follow-up to file

- Machine retention Task. Nothing removes any of these:
  - `logs/lf.log` (no rotation), `logs/terminal-*.log`, `logs/graph-*.log`,
    `logs/server.log`;
  - `runs/` capture folders, `screens/*.ghostsnp`, `holders/<retired build>/`,
    `workspace-locks/`;
  - checkout `.lf/prompts` and `.lf/journal/traces`;
  - the `session_events`, `steers`, `held_terminals`, `desktop_shells` and
    Graph run tables.
- Fixture `lf server run` processes. 31 were seen running from other
  checkouts' `target/debug` builds (LOO-932, LOO-935, LOO-916, `rewrite-skills`),
  up to 20 hours old.

## From qa (2026-11-17, private Machine)

- Acceptance check 5, run live. SIGTERM to the server, the holder and a
  provider that traps TERM and exits 0, in one `kill`: recorded died (137) and
  started again with the next server. The holder dies from TERM before writing
  `exit.json`, and an unknown end reads as SIGKILL (`held.rs:43`). SIGTERM to
  that provider alone, holder alive: recorded exit 0, read as quit, stays
  down. So a shutdown reads as quit only if the provider exits 0 and its
  holder records it before TERM reaches the holder. Whether real Claude exits
  0 on SIGTERM was not checked. The `holder_stopping` marker stays unbuilt.
- The restart count resets only on a finished turn. After a person quits and
  resumes, the count from earlier deaths stays (`running (started again 15:39,
  attempt 3)` after a person's resume), and the next death has fewer starts
  left. Is a person's resume a reset?
- A Task run's restart backoff still waits for the 60 s tick or a store
  change (the run came back 25 s after its death); conversations now wake on
  their own `next_at`.
- With the server down, a dead conversation reads `closed · starting again`;
  nothing is going to start it until a server runs.
- A recovered conversation whose provider never wrote a transcript fails each
  resume (world stand-in: exit 1, five times, then gives up). Real Claude
  writes one at its first turn, so only a death before that hits it.

## Measurements (2026-11-17, this Mac: 128 GiB, 16 cores)

- `kern.tty.ptmx_max` 511. 144 `/dev/ttys*` in use, 73 of them live held
  terminals across 8 holder processes. Each holder hosts many terminals: the
  open `held_terminals` rows all have a live holder socket.
- claude: 105 processes, 19.6 GiB RSS in total (187 MiB average), about 28 fds
  and 1.35 child processes each. codex: 135 processes, 5.8 GiB.
- 1243 processes for this user (`kern.maxprocperuid` 10666);
  `kern.maxfiles` 50000.
- Store: 21,713 `agent_sessions`, 417 `task_primary_sessions` (55 of them on
  open Tasks), 185,716 `processes` (221 unfinished).

## Failure table (from code, base `6ae67076bc`)

| Failure | Noticed by | Does |
| --- | --- | --- |
| Provider SIGKILL, holder alive | `terminal_holder.rs:634` wait, then `held.rs:212` and `hosted.rs:663`, then `native_exit::settle` | The agent row is ended. The launcher row gets `failed`/137 and the error "claude exited with 137." `held_terminals.exit_code` is set to 137 and the terminal folder is removed. `agent_sessions.completed_at` stays NULL for an interactive Task conversation |
| Holder dies | `held.rs:245` `unreachable`, `registry.rs:565` `adopt` | Settles Unknown; `held_terminals.exit_code` NULL |
| Person quits | Same path as SIGKILL | Exit 0, recorded `completed`/`succeeded`; 130 counts as interrupted |
| Graph child SIGKILL | `graph.rs:419` | `finish_run(code=None)`. `exit_of` maps the signal to 130, so the Task run ends with no retry |
| Step child killed | `graph.rs:912-983` | The Graph exits 1, and the carrier retries from the checkpoint (3 attempts) |
| Carrier killed | Nothing | The Task stays `moving` (SQL `task_state_sql`, no OS check) until a prune or reap |
| Server restart | `registry.rs:348-390` `open`/`adopt`; `wake.rs` first reading | Live holders are rehosted; holders that hold their lock but have not answered are retried with backoff |
| Reboot | `adopt` | Every terminal is settled Unknown. Graph and carrier rows stay open until a prune |

What lazy `recover()` (`ops/recovery.rs:8`) does, on Task, Wave, Plan,
history and Process reads (`bin/lf.rs:2698-2718`):
- reaps agent processes;
- `restart_ships`, for a shipping Task whose carrier is dead (backoff 1 to 30
  minutes);
- `restart_waiting_runs`;
- rental checks.

## Server lifecycle (from code)

**Startup order:**
1. Raise the fd limit.
2. Start the observe cache threads.
3. Start the transcript watchers.
4. Take `machine.lock`.
5. Unlink and bind `machine.sock`.
6. `wake::start`.
7. `statistics::start`.
8. `Registry::open`: bind `held.sock`, adopt holders.
9. Start the remote carrier, then the accept loop.

**Shutdown:** no SIGTERM handler, and nothing written on the way down.

**Supervisor settings:**
- launchd: `RunAtLoad`, `KeepAlive` and `AbandonProcessGroup`.
- systemd: `Restart=on-failure`, `KillMode=process`.

Holders are `setsid` processes, one per build, each with its own lock and
`holder-<sha12>.sock`. They die at logout and reboot.

## Slice 1 (implement, 2026-11-17)

- `agent_stopped_at` was not added: no lf command a person runs ends a held
  Task conversation's agent (no server stop operation, no Desktop close that
  ends one; `lf task interrupt` stops Graphs). Quit is read from exit 0/130.
- A conversation that died long ago still comes back on the first pass. This
  Machine has 7: one exit 137, five exit 1, one with no end since 2026-10-22.
  Passed over: ignoring deaths older than some age. That would be a second
  rule; the give-up after 5 starts already bounds a conversation that cannot
  start.
- Two branch migrations while stacked (`process_rows_on_owners.sql` from
  LOO-953, `task_conversation_recovery.sql` here). `new_migration.py` would
  have printed LOO-953's, so this one was created by hand; unmerged ones order
  by name, so it applies after LOO-953's.
- A person's `lf resume --task` racing the pass's start: both go through the
  holder, and the pass skips a Session with a live or pending terminal. A
  start in the same instant is not guarded beyond the attachment claim; not
  tested.

## Launch names (steer from LOO-924's Session, 2026-11-17)

Jack: "my suggestions: work session, worker / chat, run", then "ok, batch is
fine for bare headless". managed+typing = work session, managed+headless =
worker, bare+typing = chat, bare+headless = batch. Slice 4 docs use them.

## Slice 2 (implement, 2026-11-17)

- Conversations the pass starts get the server's environment (`continuing()`),
  not the CLI's. The old `start_in_background` passed the CLI's environment and
  `LF_WAVE_ID`; `continuing()` strips every `LF_` variable, as for a phone's
  continue. Not checked: whether a resumed Task conversation in a Wave still
  resolves that Wave's settings without it. A provider found only on a shell's custom `PATH` and not
  in `supervised_path` now records "not installed" instead of starting.
- Desktop's open of a Task conversation and the pass can race on a prepared
  Session. Both go through the registry; `run_refusal` refuses a second live
  agent. Not tested.
- Settlement runs at most every 10 s, so a dead ship carrier restarts within
  about a minute (60 s unread tick) when nothing else writes.
- A terminal that outlives its provider (the launcher lingering after a
  provider death) keeps the pass re-reading every second until it ends.

## Slice 3 (implement, 2026-11-17)

- A carrier that catches SIGHUP (its terminal closed) records an interrupted
  outcome itself, so it reads as a person's stop and is not carried again.
  ctrlc's hook does not say which signal arrived. If closing a terminal under
  `lf task run` should count as a death, the carrier would need to record the
  signal.
- A person's own `lf task run` after the give-up does not reset
  `task_graphs.restarts`; only a finished step does. One more death right
  after would wait again without a restart. Chosen because the restart and a
  person's run are the same command.
- The died line reuses `waiting` (documented for limit waits) while a restart
  is due, and `blocked` after giving up.

## Prune into the pass (implement, 2026-11-17)

- Retention runs at most hourly, starting with the pass's first reading. This
  Machine's store has never been pruned on a schedule (185,716 `processes`), so
  the first retention after this ships may VACUUM; it runs after that
  reading's conversation starts, but it holds the recovery thread until done.
  Not timed at this Machine's size.
- `lf process prune --dry-run` had no replacement: there is no way to preview
  what the pass will settle. `lf machine doctor`'s lineage line still counts
  repairable ancestry.

## Slice 4 (implement, 2026-11-17)

- The concept-review answers "1) a 2) a 3) b" were applied as the steer read
  them (1 naming, 2 carrier count, 3 wait for LOO-953). In
  `scratch/concept-review.html` question 2 is what resets the attempt count;
  the reset was left as built (any `yielded`), so its counterexample still
  stands: if a resumed provider yields at startup, the 5-start limit never
  binds.
- `ended` describes the current agent only, so a conversation running again
  after a death shows `start.attempts` but not the earlier exit code.
- `start` is computed for any open Task's primary Session, not only one
  checked out here; a Task checked out elsewhere could read "starting again"
  that no pass here will do.
- A ship carrier's restarts now count on `task_graphs.restarts`; a shipping
  Task that later stops shipping keeps that count until a step finishes, so it
  could read "died 5 times" sooner. Ships themselves still never give up.
- `next_at` with no prior start is the death time (immediate), so a death the
  pass is about to start reads "starting again" even while the pass waits for
  the dying terminal to clear.

## Concept review after qa (2026-11-17, `scratch/concept-review.html`)

Choices made for review, built in slice 5:
- 1a: a person's `lf resume` of a died conversation resets the restart count.
- 2a: with no server running, the line says "starts again when the server runs".
- 3a: a died conversation with no transcript is left as is (recorded only).
