# LOO-824 open choices

Exploration choices (1b, 2a, 3a, 4a) are answered; the quotes and the
direction built from them are in `holder.md`. Options as offered:
`a18d6a915:scratch/questions.md`.

## Assumptions, reversible

- Desktop Task shells stay on their own `session serve` owner until LOO-830.
- No snapshot on graceful stop: rebuild is ring replay plus a nudge;
  terminal-mode recovery after a gap is unverified (slice 0).
- Ring 2 MiB per PTY; holder exits 60 s after its last child.
- Store names are `held_terminals` / `held_terminal_id`: `processes.terminal_id`
  already holds Desktop pane placement. The docs say "terminal" throughout.
- Plain CLI views and the held child's control requests reach the registry
  through `held.sock` directly; the `session.attach` ticket bridge serves
  clients that speak only the server protocol (Desktop).
- `bridged` and the attach ticket carry both `owner_lf_process_id` and
  `terminal_id`, one of them null, instead of two shapes under one name.
- A lost `session.start` holder reply answers `outcome_unknown`; the caller
  repeats the same `request`.
- `session.attach` for a pending holder's terminal answers `unavailable` with
  `retry: true`: the holder is a dependency not answering, not the server at
  capacity. No client reads `retry` yet.
- A replaced view (another `held.sock` revision) does not carry `--moved` or
  `--session-json` into the installed `lf`.
- Nothing yet lists which conversations a killed holder took.
- Task cleanup that cannot reach the server for a held terminal logs a
  warning and leaves it a candidate for the next pass, rather than failing
  the pass for the Task's other conversations.

## Holding a conversation reopened by id

Fresh launches hold by the command (`task_run || cli.names_work()`); a reopen
by id alone holds, and needs the server, when its row is `declared` or
`bound` (`transport::held`, `agent_process::require_server_for`). Jack Heart:
"i think (1a) is right, so double coverage for now". Desktop's pane reopen
already names its Task or Wave (LOO-821 on main); a Session link opens by id.

## Stack parent not recorded

`lf pr publish` retargets #1887 to `main` each time; after each publish set
`gh pr edit 1887 --base jack/loo-823-find-what-leaks-lf-processes-and-fix-the-tests`
(found on `main` and reset at realign, Nov 11).

## Stack on LOO-823 (November 11)

Shown `scratch/simplification-opportunities.md` (opportunity 1: sync onto
LOO-823 now), Jack Heart: "yes, you can stack on 823". Then: "821 is landed"
(PR #1879 merged 2026-11-11 19:12 UTC, 763da5c92). New stack: main ← LOO-823
(PR #1877) ← LOO-824.

## Resident-limit suspension, handed over from LOO-823 (November 11)

LOO-823's Session (comment b9d25c20) moved suspension of idle managed
conversations to LOO-824, quoting Jack Heart: "i could be open to allowing
soem sort of "idle" state for task and wave/repo sessions on the server, if we
ahve some cache limit, as long as only cost is slightly slower spin up time
next time youj switch to them"; "6a" (a per-Machine agent count in config,
default unnamed); "no opinion on 5" (read an unsent draft off the server
emulator before suspending). An earlier LOO-824 steer (8606000f) says
suspension is not in this Task. Undecided: here or its own Task.
Asked here, own Task / in LOO-824 after this demo / backlog; Jack Heart:
"1b" (in LOO-824, after this demo).

## LOO-823 sync resolution (November 11)

Keep LOO-824’s review and open choices; LOO-823’s historical notes remain in
`ed74fefaa:scratch/process-leaks.md`. Bare-direct applies only when neither
the command nor the reopened Session names managed work. Existing ongoing
conversations still keep their host when their older row has no WorkSource.

## Demo (November 11)

Jack Heart: "i dont need to demo", then "2c, then maybe 2b" (walk through `scratch/qa.html`, then maybe an isolated `LF_HOME` run).
