# LOO-958: one verb per want; steer wakes a stopped Session

Direction: `scratch/direction.md` (exact words). Full command table and open
questions: `scratch/session-commands-review.html`. This plan builds the first
slice only; questions 3–5 there (open vs resume, fresh-thread verb, selectors,
the `move` words) are each decided before they are built.

## Outcome

`lf steer SESSION 'text'` reaches a Session whether or not its agent is
running. Running, it reads the steer at its next tool call, as today. Idle and
held by the server, the server wakes it, as today. Stopped, the steer starts
one headless turn there. `lf -b steer SESSION 'text'` waits for that turn and
prints the reply. `lf resume` loses `--message` and only connects.

Only a steer naming one Session wakes. Task, Wave and repository steers and
Task updates never start a process: "if you steer a task the task conversation
should ~always be awake anyway and it can continue from there".

## Demo

```bash
lf : 'remember the word mango'            # a Session, then quit it
lf steer S 'what word did I give you?'    # "Saved <steer>; woke S" — returns
lf session steers S                       # the steer, delivered
lf session history S                      # a new turn answering "mango"
lf -b steer S 'spell it backwards'        # waits, prints "ognam"
lf -b resume S --message hi               # error: unexpected argument '--message'
```

## Acceptance at gate

`rust/loopflow/tests/session_steers.rs`, with the Claude stand-in and a
`MachineServer`: a stopped interactive Session receives `lf steer S TEXT`; the
command returns before the turn ends; the stand-in sees exactly one turn whose
prompt carries TEXT in full; `lf session steers S --json` shows it delivered.
Two `lf steer` calls racing on the same stopped Session start one woken process
that delivers both. The Graph structured-output correction test in
`session_launch_contract.rs` passes through `steer`.

## What the code says (findings beside the decisions they drive)

**Stopped vs closed.** No state column says "stopped". A Session is *running*
when `session_summary` shows no `completed_at` and its AgentProcess identity is
`Live` (`ops/human_session.rs` `run_refusal`). Headless Sessions
(`interactive = 0`, a *Run* since LOO-924) get `completed_at` when their
AgentProcess exits (`migrations/complete_finished_headless_sessions.sql`);
interactive Sessions never complete. So:

- interactive, no live AgentProcess → stopped → a named steer wakes it;
- a Run (non-interactive) → woken only by its own Graph (the correction loop),
  the same exception `run::resume` already makes with `caller_graph`/`own_step`.
  Anyone else's steer to a finished Run is saved and `lf steer` says nothing
  will read it until the Run's Graph resumes it. *Made for review:* the
  alternative was letting any named steer wake a finished Graph step; passed
  over because a Run's checkout belongs to its Graph's next step.

**Who starts the process.** The server runs no process per Session
(`server/wake.rs` header) and only wakes *live* idle conversations, through
Claude's inbox socket or Codex's app-server (`steer::idle_conversations`
filters on `Live`). A stopped Session has no socket to write to, so a wake is a
new provider process. The command that saved the steer starts it with
`os_process::detached::start_lf`, the way `lf task run` and `pr land` start
their workers (`ops/task/ship.rs`, `ops/pr_landing.rs`). The wake goes in
`engine::session::steer`'s `SteerScope::Session` arm, so `lf steer` and
Desktop's served `session.steer` (`swift/Loopflow/Services/RegistryQuery.swift`
`.sessionSteer`) share it. The detached child is a hidden
`lf -b session wake ID` ("Run a stopped Session's unread steers as its next
turn; `lf steer` starts it"), following the hidden `lf task run` → `Run`
precedent. *Made for review:* the alternative, re-running the caller's own
argv detached, has no argv when the server executes `session.steer`.

**The woken turn.** It is today's `run::resume` (`lf/commands/run.rs`) with the
message replaced by the Session's unread steers: same launch Run
(`Bare(Headless)` when detached, `Worker` inside a Graph step, both from
`transport::delegate_current`), same harness-switch continuation, same
`--output-schema`. The prompt carries each steer's attribution line and its
**complete** text, not the hook's 400-byte excerpt (`steer::render`): the
Graph correction's text includes `output.instructions()` and must arrive
whole. The steers are claimed before launch through `steer::claim`'s claim and
lapse, and marked delivered once the turn starts, so the turn's own hooks don't
inject them a second time and a waker that dies mid-launch loses nothing.

**One process per wake.** `lock_session_process` is held only through workspace
admission (`lf/commands/session.rs` `resume`), so two `--message` calls today
start two turns. The waker takes its own non-blocking lock per Session
(beside the launch lock in `lf_home/…`): a second waker that finds it held
exits; the first, after each turn, runs another while undelivered steers
remain, then releases the lock and checks once more, which closes the race
with a steer saved while it held the lock. The steering command also skips the
wake when the Session is running. Not the launch lock itself: holding it for a
turn would make `lf resume S` fail with "Session launch is still busy".

**A woken turn beside a live Graph step.** An interactive Session bound to a
Task shares its checkout with that Task's Graph steps. `lf resume` of that
Session already does the same; the wake accepts the context and adds no guard.

**Graph correction.** `graph.rs` (the "corrected in the same conversation"
loop) spawns `--batch --output-schema S session resume CONV --message TEXT`
and reads the answer from the step's captured input. It becomes
`--batch --output-schema S steer CONV TEXT`; `-b` runs the wake in the step's
own process, so `self.answer` reads the same capture. The steer's author comes
from `participant_name_in`, as for any agent-run `lf steer`.

**Delivery failures.** A provider that cannot start leaves its steers unclaimed
(after the lapse) and visible in `lf session steers`; the detached waker logs
to `logs/lf.log`. `lf steer` returns 0 once the steer is saved; a failed wake
start prints one line ("saved; S did not start: …") and still exits 0, since
the steer is the thing it was asked to save. `lf -b steer` exits with the
turn's failure.

## What goes

- `SessionConnectionArgs::message` (`lf/mod.rs`) and every `conflicts_with =
  "message"`; the `--message` branch of `session::resume` and its
  "a headless connection requires --message TEXT" / "a next-turn message
  requires -b" checks; `session new`'s "continue it with `lf -b resume
  --message`" refusal and the `--prepare` refusal of a message.
- `lf -b resume` (no `--message`) becomes an error naming `lf -b steer`.
- `Cli::resume`'s doc comment naming `--message`.
- The `run_refusal` text "`lf resume` takes only Sessions" stays for `resume`;
  `run::resume` moves behind the wake.
- Tests and callers: `session_lifecycle_tests.rs` (390, 1134–1141),
  `served_reads.rs` (2396–2595), `context_launch_tests.rs`,
  `session_launch_contract.rs`, `session_cli_tests.rs`, `transport_tests.rs`,
  the `lf/mod.rs` parser test at 4081, `python/tests/test_build_provenance.py`,
  `scripts/benchmarks/pi/probe.py` (no-ID `-b resume --message` becomes
  `lf -b :`), `scripts/benchmarks/crate-rebuild/*.py`.
- `public_commands_save_steers_without_launching_providers` keeps its
  Task/Wave/repository half; its stopped-Session half now expects one wake.
- Docs: `docs/sessions.md`, `docs/config.md`, `docs/dev/execution.md`,
  `docs/dev/planning.md`, `docs/dev/reference-models.md`,
  `.lf/qa/provider-switching.md`, and `lf steer`'s Clap help ("No turn
  starts" becomes "A stopped Session named by ID starts its next turn").
- Release notes under `release/` are shipped records and stay.

## Slices

1. **Wake in the foreground.** `lf -b steer S TEXT` on a stopped interactive
   Session: save, take the wake lock, claim, run `run::resume` with the full
   steer text, deliver, print the reply. Graph correction switches to it and
   `--message` is deleted in the same slice, with its tests moved. This proves
   or kills the plan: whether claim-before-launch composes with the woken
   turn's own hooks (no second injection), and whether the correction still
   reads its answer from the step capture.
2. **Detached wake.** `lf steer S TEXT` and served `session.steer` start
   `lf -b session wake S` detached and return; the wake loop and its lock;
   finished-Run refusal message; racing-steers test.
3. **Docs and help** across the files above; `.lf/qa/provider-switching.md`
   drives `steer` in place of `resume --message`.

## Excluded

`open` vs `resume`, selectors, `session restart`/`fork` verbs, `move` words,
`--prepare`: later slices, each designed first. Waking on `--machine` routing
runs wherever the command already runs; no new routing.
