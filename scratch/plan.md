# LOO-958: one verb per want; steer wakes a stopped Session

Direction: `scratch/direction.md` (exact words). Full command table and open
questions: `scratch/session-commands-review.html`. Concept review of this plan:
`scratch/concept-review.html`. This plan builds the first slice only;
questions 3–5 on the commands page (open vs resume, fresh-thread verb,
selectors, the `move` words) are each decided before they are built.

## Outcome

`lf steer SESSION 'text'` reaches a Session whether or not its agent is
running. Running, it reads the steer at its next boundary, as today. Idle and
held by the server, the server wakes it, as today. Stopped, the Machine server
starts it again in a held terminal, the way the phone's Continue does, and
then wakes it like any idle held conversation. `lf resume S` afterwards
attaches to that terminal and shows the answer. `lf resume` loses `--message`
and only connects.

`lf -b steer SESSION 'text'` is the foreground form, like `lf -b task run`:
the turn runs headless in the caller's process and the reply prints. Graph
correction uses it.

Who a steer wakes:

- a steer naming a Session wakes that Session;
- `lf steer TASK 'text'` wakes the Task's primary conversation and its worker,
  as a steer naming each would: "steering a task should behave like steering
  both the worker session and the main task session", "i mean it WAKES
  those". Every running Session of the Task still reads it at its next
  boundary; the Task's other stopped Sessions are not woken;
- Wave and repository steers and Task updates wake no stopped Session.

*Chosen for review:* the worker, when no Graph step is running, is nobody. The
primary conversation is woken and decides what happens next, including `lf
task run` or a steer naming a step: "the task conversation ... can continue
from there". Passed over: holding the steer for the next step (it already
reads it at its first boundary) and waking the last finished step (that
re-runs work in the checkout).

## Demo

```bash
lf : 'remember the word mango'            # a Session, then quit it
lf steer S 'what word did I give you?'    # "Saved <steer>; the Machine server starts S to read it"
lf session steers S                       # the steer, delivered
lf resume S                               # attaches to S's held terminal; the answer is there
lf -b steer S 'spell it backwards'        # (after quitting) runs here, prints "ognam"
lf -b resume S --message hi               # error: unexpected argument '--message'
```

## Acceptance at gate

`rust/loopflow/tests/session_steers.rs`, with the Claude stand-in and a
`MachineServer`: a stopped interactive Session receives `lf steer S TEXT`; the
command returns at once; the server starts one held terminal for S; the
stand-in sees one turn whose input carries TEXT; `lf session steers S --json`
shows it delivered. Two steers saved together to the same stopped Session
start one held terminal that delivers both. A Task steer to a Task whose
primary conversation is stopped starts that conversation and no other Session
of the Task. `lf -b steer` on a stopped Session prints the reply. The Graph
structured-output correction test in `session_launch_contract.rs` passes
through `-b steer`.

## What the code says (findings beside the decisions they drive)

**Stopped vs closed.** No state column says "stopped". A Session is *running*
when `session_summary` shows no `completed_at` and its AgentProcess identity is
`Live` (`ops/human_session.rs` `run_refusal`). Headless Sessions
(`interactive = 0`, a *Run* since LOO-924) get `completed_at` when their
AgentProcess exits (`migrations/complete_finished_headless_sessions.sql`);
interactive Sessions never complete. So:

- interactive, no live AgentProcess → stopped → a steer that wakes it starts it;
- a Run (non-interactive) → only its own Graph continues it, through `-b
  steer`, the same exception `run::resume` makes with `caller_graph`. The
  server never starts a Run. Anyone else's steer to a finished Run is saved
  and `lf steer` says nothing will read it until its Graph continues it.

**One waker: the Machine server.** `server/wake.rs` already owns waking: one
watcher reads the steer and Task-update revisions and wakes each idle live
held conversation it has news for (`steer::idle_conversations`, reach
`IdleReach::Claude` inbox or `IdleReach::Codex` app-server). A stopped
Session is the same news with no process to reach. The watcher gains that
case: an interactive Session with an undelivered steer that wakes it and no
live AgentProcess is started with `session.continue`'s launch
(`server/service.rs` `continuing` → `Registry::start`), which runs `lf
session resume -- S` in a held terminal in its recorded directory, refusing
through `run_refusal` a Session that is already running. Once the resumed
conversation yields, the existing idle path delivers the steer. The steering
command (`engine::session::steer`, so `lf steer` and Desktop's served
`session.steer` alike) only saves.

This replaces the kickoff's detached `lf -b session wake ID` started by the
steering command, its per-Session wake lock and its run-again loop. Each was
a second copy of something the watcher has: one reader of news, `run_refusal`
against a second process, and a reread when a turn ends (`wake::after_turn`).

**Each steer starts at most one process.** The watcher's first reading after
a server start wakes everything with news ("A server restart forgets
nothing"). With stopped Sessions in scope, a Machine restart would start every
interactive Session holding an old undelivered steer. So a start is recorded
per steer: the held start's `request` is the steer ID, and the steer row
records that it started its Session once (new nullable column, one migration).
A steer that already started its Session once waits for a normal boundary.
The migration marks every existing undelivered steer as already started, so
upgrading wakes nothing old.

**Without a server.** The steer is saved and visible in `lf session steers`;
the server's first reading starts the Session. `lf steer` says which of three
things happens: S reads it at its next boundary (running), the server starts
S to read it (stopped), or nothing reads it until its Graph continues it (a
finished Run).

**The foreground turn (`-b steer`).** It is today's `run::resume`
(`lf/commands/run.rs`) with the message replaced by the Session's unread
steers: same launch Run (`Bare(Headless)` outside a Graph, `Worker` inside a
Graph step, both from `transport::delegate_current`), same harness-switch
continuation, same `--output-schema`. The prompt carries each steer's
attribution line and **complete** text, not the hook's 400-byte excerpt
(`steer::render`): the Graph correction includes `output.instructions()`. The
steer is saved already claimed by this process (one store write), so the
watcher never sees it as news and starts no held terminal beside it; it is
marked delivered when the turn starts. Steers saved during the turn arrive
through hooks at its boundaries; one saved after the last boundary is news
for the watcher once the process exits. `-b steer` to a running Session saves
and returns: there is no turn of its own to wait for.

**One process per Session.** `lock_session_process` is held only through
workspace admission (`lf/commands/session.rs` `resume`). The foreground turn
takes it and checks `run_refusal` under it before launching, like
`continuing` does, so a held start and a foreground turn can't both launch.
Verify in slice 1 that the AgentProcess identity is recorded before the lock
is released; if not, recording it inside the lock is the fix, and it also
closes the same race for two `lf resume` calls.

**A woken Session beside a live Graph step.** An interactive Session bound to
a Task shares its checkout with that Task's Graph steps. `lf resume` of that
Session already does the same; the wake accepts the context and adds no guard.

**Graph correction.** `graph.rs` (the "corrected in the same conversation"
loop) spawns `--batch --output-schema S session resume CONV --message TEXT`
and reads the answer from the step's captured input. It becomes
`--batch --output-schema S steer CONV TEXT`; `-b` runs the turn in the step's
own process, so `self.answer` reads the same capture. The steer's author comes
from `participant_name_in`, as for any agent-run `lf steer`.

**Delivery failures.** A held start that fails is logged by the server to
`logs/lf.log` and leaves the steer undelivered and visible; it does not retry
on its own (the steer has used its start). `lf steer` exits 0 once the steer
is saved. `lf -b steer` exits with the turn's failure.

## What goes

- `SessionConnectionArgs::message` (`lf/mod.rs`) and every `conflicts_with =
  "message"`; the `--message` branch of `session::resume` and its
  "a headless connection requires --message TEXT" / "a next-turn message
  requires -b" checks; `session new`'s "continue it with `lf -b resume
  --message`" refusal and the `--prepare` refusal of a message.
- `lf -b resume` (no `--message`) becomes an error naming `lf -b steer`.
- `Cli::resume`'s doc comment naming `--message`.
- The `run_refusal` text "`lf resume` takes only Sessions" stays for `resume`;
  `run::resume` moves behind `-b steer`.
- Tests and callers: `session_lifecycle_tests.rs` (390, 1134–1141),
  `served_reads.rs` (2396–2595), `context_launch_tests.rs`,
  `session_launch_contract.rs`, `session_cli_tests.rs`, `transport_tests.rs`,
  the `lf/mod.rs` parser test at 4081, `python/tests/test_build_provenance.py`,
  `scripts/benchmarks/pi/probe.py` (no-ID `-b resume --message` becomes
  `lf -b :`), `scripts/benchmarks/crate-rebuild/*.py`.
- `public_commands_save_steers_without_launching_providers`: the command
  itself still launches nothing; its stopped-Session and Task halves move to
  the server wake tests.
- Docs: `docs/sessions.md`, `docs/config.md`, `docs/dev/execution.md`,
  `docs/dev/planning.md`, `docs/dev/reference-models.md`,
  `rust/loopflow/src/server/README.md` (the watcher's third reach; `session.continue`
  is no longer the only server-started process besides `session.start`),
  `.lf/qa/provider-switching.md`, and `lf steer`'s Clap help ("No turn
  starts" becomes "A stopped Session it wakes is started again by the Machine
  server; `-b` runs the turn here and prints the reply").
- Release notes under `release/` are shipped records and stay.

## Slices

1. **Foreground turn.** `lf -b steer S TEXT` on a stopped Session: save
   claimed, take the launch lock, `run_refusal`, run `run::resume` with the
   full steer text, deliver, print the reply. Graph correction switches to it
   and `--message` is deleted in the same slice, with its tests moved. Proves
   or kills: claim-at-save composes with the turn's own hooks (no second
   injection); the correction still reads its answer from the step capture;
   the AgentProcess is recorded inside the launch lock.
2. **The server starts stopped Sessions.** The watcher's stopped case through
   `continuing`; the per-steer start column and its migration; Task steers
   naming the primary conversation (and a running worker) as wake targets;
   `lf steer`'s three-way message. Proves or kills: a `lf session resume` in a
   held terminal yields as idle without a viewer, so the existing inbox and
   app-server wakes deliver. If it doesn't, the held start passes the steer as
   the resumed conversation's first input instead.
3. **Docs and help** across the files above; `.lf/qa/provider-switching.md`
   drives `steer` in place of `resume --message`.

## Excluded

`open` vs `resume`, selectors, `session restart`/`fork` verbs, `move` words,
`--prepare`: later slices, each designed first. Waking on `--machine` routing
runs wherever the command already runs; no new routing.
