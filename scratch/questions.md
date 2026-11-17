# LOO-936 open choices

- History retention: 30 days for finished Processes, 90 days for Session
  history. Chosen so weekly usage and recent Task history keep working;
  either number is one constant. Jack Heart, 2026-11-16: "Seems fine".
- The attachment-token fence on history writes reads `loopflow.db` without
  its write lock, so a replaced agent can append one stray history row after
  a handoff. Accepted because history grants no authority.
- `output` (the last assistant message): Jack Heart, 2026-11-16, chose
  option B: "(2) (B)". Stop copying `output` into the store; its readers
  (Graph routing `graph.rs:975`, `lf run` `run.rs:836`, ci-fix
  `pr_landing.rs:481`, Task view, `session_history`, `fact_writer.rs:846`)
  read the last assistant message from the provider transcript when they
  need it. Risks to design for: Claude and Codex transcript formats and
  locations change; transcripts can be compacted, moved or deleted; a parse
  failure must not silently pass a step (fail it with a clear error naming
  the transcript path, and log it); one reader per provider behind one
  function. Existing `output` rows get no new writes and are deleted under
  retention, not moved.
- Where Process rows live. Jack Heart, 2026-11-16: "i guess i wonder if all
  process info should just be in a separate db/", then, after the prior-art
  comparison (Kubernetes: pod status with leases, events with TTL in a
  separate etcd, records keep their own status; Slurm: slurmdbd accounting
  apart from the controller): "ok. kube and slurm seem like reliable prior
  art." Direction: every Process row, live and finished, lives in the
  Machine-history file; nothing moves at exit. This settles the `lf history`
  question as (C): records in `loopflow.db` keep the results their readers
  need on their own row.
- The plan calls the retention pass "the existing hourly `lf top --prune`
  pass", but nothing in the repo schedules `lf process prune` (only manual
  runs and `scripts/disk-cleanup.sh` paths). Docs say retention applies when
  `lf process prune` runs and make no hourly claim. Asked whether the Machine
  server should run it on a clock; Jack Heart, 2026-11-16: "Yeah that woudl
  be good". Slice 5 adds the server's clock and the docs state it.
- `has_provider_history` (slice 1) drops only its `provider_session_observed`
  arm and keeps reading the current input's `provider-session:*` row, not
  `agent_sessions.provider_thread`: a new input must not borrow an older
  input's resume reference (`recovery_refresh_recognizes_imported_history_before_attachment`).
  Settled in the design: the per-input thread becomes a column on the
  `captured` row in main, so it never reads the history file.
- The two "Revised direction" questions (attachment placement; Task state
  reading the history file). Jack Heart, 2026-11-16: "(1) on the session (2)
  scond": the attachment token and attached Process move to `agent_sessions`;
  Graph and ship records carry their own `ended_at`, outcome and error, and
  the liveness check stamps them for a Process that dies without exiting.
- Process rows in the history file. Jack Heart, 2026-11-16, on the `outcome`
  column: "i just wanted to flag to you as somethign that needs a better
  design" and "and like maybe lf processes and agent processes shouldnt be in
  the same table?" Direction (steer): one table per kind (LfProcess,
  AgentProcess), only the columns its kind fills and a reader uses, no
  `outcome` column (derive from exit_code and error, also on Graph and ship
  records), and say how an agent Process records how it ended. Table
  definitions and each column's reader go in the plan before slice 4. Another
  branch stops writing `processes.outcome` for agent rows; whichever lands
  second reconciles. Also to design before shipping: what a live Session on
  the released lf sees after install, while it still reads and writes
  `processes` and the attachment token in `loopflow.db`.
- Slice 3 choices (headless, reversible): the independent flag is a column
  on the input's captured row, not `agent_sessions` (it is per input; a new
  input resets it by having none); the resume account is
  `session_events.provider_account` beside `provider_thread`, since
  `agent/session.rs` resumes on it; the Session feed lists history rows only,
  because one feed over two files' sequences cannot stay append-only; a
  rented Machine's record copies carry only captured rows, not its history.

## Machine input outage, 2026-11-16

Jack Heart: "almost or all of my sessions seem to not be receiving input anymore";
clarification: "No typed characters appear". Rex screenshot showed multiple
Claude and Codex Sessions. Direction after considering restarts: "well if the
machine server is the issue", "then i guess do somethign eslse".

At 17:58 PST, installed `lf server status` reported `ready:false`,
`listening:false`; Dev Desktop showed stale Session screens and "Loopflow server
is not running". launchd's `com.loopflow.server` had exit 78 and a cached
Executable `/Applications/Loopflow.app//Users/jack/.lf/performance/live/20261116-workflows/lf-b7e820d1f`,
while the bundle plist named `Contents/MacOS/lf`. `lf install server` returned
unavailable twice; unloading the stopped job did not clear SMAppService's
registered status. macOS App Management refused adding a temporary registration
helper to `/Applications/Loopflow.app`; no bundle files changed.

Restored service with launchd job `com.loopflow.server.recovery`, definition
`~/.lf/server-recovery.plist`, using the installed
`/Applications/Loopflow.app/Contents/MacOS/lf server run`; output goes to
`~/.lf/logs/server-recovery.log`. At 18:02 PST readiness was true, instance
`6b911057-f564-4159-ae01-c210f09e4ebd`, revision 5; Desktop reconnected.
No Rex Sessions or provider processes were restarted. This job lasts until
logout/reboot; repair the bundled SMAppService registration, then unload the
recovery job. Whether this restores Rex typing is awaiting a reply.

## Whether Process rows are needed at all, 2026-11-16 (undecided; run stopped)

Jack Heart, in order, after the per-kind table draft:
- "do we need both?" (the `lf_processes` and `agent_processes` tables)
- "what are hte process tables for"
- "should that be proc.db then or something"
- "now that we've moved to having a machine server and processes dont even
  map 1:1 with agent sessions i am unconvinced something like lf top and lf
  ps even really makes sense"
- On "A Machine-wide view of what's running ... becomes a list of live
  Sessions and Graph runs from the server, not a Process tree": "Check"

Measured: 21,247 Sessions, 22,443 agent rows; 20,607 Sessions have exactly one
agent Process; no Session has two live; no agent row is the parent of an `lf`
row. In builtin skills `lf ps --json` is called only by the three operate
skills, to check whether a Graph in status is alive.

The three jobs of `processes`: a registry of what is running (liveness,
reaching an agent, reaping); the result of something another record points at
(now carried by Graph and ship records); a log of what ran (`lf history`,
`lf context --process`, `lf doctor`).

Open: (a) slice 4 becomes "liveness on Graph runs and Sessions, no Process
tables", or LOO-936 lands at slice 3 and that is its own Task; (b) whether
`lf history` stays, as a log or at all; (c) bare `lf -b` runs, which the
server does not hold.
- On bare `lf -b <skill>` runs that the server does not hold: Jack Heart:
  "These should probably register themselves in some db somewhere"
- Jack Heart: "maybe that is the process table all over again hmm" (on a
  self-registering registry of runs), then "on the other hand can we just
  filter regular lf to ps?" (read as: the OS process list, filtered to lf's
  processes), then "go for it" (check it).

### What the OS process list showed, 2026-11-16 18:40 PST (this Machine, macOS)

Method: `lf ps --json` pids, then `ps eww -o command= -p <pid>` for each and
`ps axeww` for the reverse, reading only `LF_*` variable names. Run from an
agent's Bash with no sandbox wrapper (parent is `claude` directly), so a
sandboxed read is untested.

- `lf ps` listed 219 Processes; all 219 were alive in the OS (0 stale).
- Agent processes carry their Session: `LF_STEER_SESSION` on 46 of 46
  `codex -c` and 23 of 24 `claude --name`. `LF_PROCESS_ID` is the launcher's
  id, inherited, not the process's own (2 of 46 codex, 24 of 24 claude).
- 21 Processes show no `LF_*` variable: all 8 `lf holder` and 13 `lf`. A
  process started from a person's shell learns its id after it starts, and
  the OS reports only the environment it was started with.
- 278 OS processes carry `LF_PROCESS_ID`, `LF_TRACE_ID` or
  `LF_HUMAN_SESSION`; 128 are not in `lf ps`: children that inherited the
  variables (`browse` 50, `codex-code-mode-host` 15, python 24, `claude` 10,
  `codex` 10, `uv` 4).

So the environment names the Session an agent serves and over-matches its
children, and names nothing for an `lf` a person started. It can label and
find strays; it cannot be the only source of "which run is this".

- On the design "rows are the source (pid and start time on the Session or Graph run), the OS is the check, no registry and no Process table, environment scan as a diagnostic": Jack Heart: "all seems fine."

- On `lf history`: Jack Heart: "what im most interested is being able to investigate things like tokens and time by wave or by skill or other things like that"

- Jack Heart: "i do wonder if the per-turn session rows (which can be useful also for like group-by-model) should also go in a separate db"

- On the three problems for usage investigation (90-day retention cutting off trends; token counts inside JSON with start and end as separate rows; model recorded on 6,261 of 21,338 Sessions): Jack Heart: "Agree we want to fix tthese"

## Calls made to relaunch, 2026-11-16 (each one a constant or a section to change)

- Process removal is its own Task, not slice 4 here. Offered twice with "I'd
  take the second"; the reply to the message carrying it was "all seems fine."
- Per-turn rows go in `usage.db`, self-contained. Raised as "i do wonder if
  ... should also go in a separate db"; not confirmed in words.
- Turn rows kept with no retention; grouping values frozen at turn end with
  the Session id kept. Neither was answered.
- `lf history list` is untouched here; whether it goes is decided with the
  Process table in the other Task.
- Turn grouping (concept review, headless): `usage.turns` takes Task, Wave,
  skill and Graph step as frozen at dispatch, because the code already
  freezes a turn's origin there (`record_session_turn_origin`), not "at turn
  end" as the direction said; the two differ only for a bind made mid-turn.
  Still open: whether reports default to these frozen values or follow later
  binds through the Session id.
