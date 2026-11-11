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

## Holding a conversation reopened by id (answered below)

LOO-821 (`1217ff206`) now decides managed by the command (`Cli::names_work`,
`AgentConfig::managed`) and deleted `WorkSource::is_managed`; its notes quote
"the cli comamnd used should direct down different codepaths". LOO-824's
steers say "managed should = held" and "repo and wave sessions should be
managed". `lf session resume <id>` (how Desktop reopens a conversation)
names no Work, so a pure command rule leaves a cold reopen of a Task
conversation unheld.

Taken (reversible): fresh launches hold by the command
(`task_run || cli.names_work()`); reopening a recorded conversation holds,
and needs the server, when its row is `declared` or `bound`
(`transport::held`, `agent_process::require_server_for`). `is_managed` stays
for that one read. Alternative: thread the resume target into
`human_session::open` and hold only `--task`/`--wave`/repository targets,
leaving `resume <id>` unheld.

## Stack parent not recorded

`lf sync --plan` reads base `main` after `lf sync
jack/loo-821-managed-sessions-hooks-call-the-machine-server`, and `lf pr
publish` retargets #1887 to `main` each time. Base set back to LOO-821's
branch with `gh pr edit --base`; the next publish will move it again.

## Stack on LOO-823 (November 11)

Shown `scratch/simplification-opportunities.md` (opportunity 1: sync onto
LOO-823 now), Jack Heart: "yes, you can stack on 823". Then: "821 is landed"
(PR #1879 merged 2026-11-11 19:12 UTC, 763da5c92). New stack: main ← LOO-823
(PR #1877) ← LOO-824.

## Reopen by id: answered (November 11)

Jack Heart: "seems fine to change how deskto opens a pane" / "But also i
think (1a) is right, so double coverage for now". Kept: the row read in
`transport::held` and `require_server_for`. Desktop naming the Work on reopen
is a separate Task.
Then: "but you can just change the desktop reopen code directly" / "i dont
think it needs a separate task". The separate Task (LOO-846) is deleted; the
Desktop change is in LOO-824.
