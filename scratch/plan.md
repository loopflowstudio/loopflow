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
starts it again in a held terminal, the way the phone's Continue does, and the
resumed conversation takes its unread steers as its first input. `lf resume S` afterwards
attaches to that terminal and shows the answer. `lf resume` loses `--message`
and only connects.

`lf -b steer SESSION 'text'` is the foreground form, like `lf -b task run`:
the turn runs headless in the caller's process and the reply prints. Graph
correction uses it.

Who a steer wakes:

- a steer naming a Session wakes that Session;
- `lf steer TASK 'text'` wakes the Task's primary conversation (and a running
  worker reads it at its next boundary), as a steer naming each would: "steering a task should behave like steering
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
through `run_refusal` a Session that is already running. A resumed provider
never yields without input, so the resume hands the conversation its unread
steers as its first input (`take_steers_to_start`). The steering
command (`engine::session::steer`, so `lf steer` and Desktop's served
`session.steer` alike) only saves.

This replaces the kickoff's detached `lf -b session wake ID` started by the
steering command, its per-Session wake lock and its run-again loop. Each was
a second copy of something the watcher has: one reader of news, `run_refusal`
against a second process, and a reread when a turn ends (`wake::after_turn`).

**Each steer starts at most one process.** The watcher's first reading after
a server start wakes everything with news, so a start is recorded once per
steer: `steer_wake_reading` is a one-row cursor over `steers.seq`
(`read_steer_wakes` reads the range past it), so nothing is stamped on
`steers`, whose triggers move the planning revision. The migration sets the
cursor past every existing steer, so upgrading wakes nothing old.

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
steer is saved and every unread one marked delivered in one write
(`take_session_steers`), so the watcher starts no held terminal beside it and
the turn's hooks print nothing twice. Steers saved during the turn arrive
through hooks at its boundaries; one saved after the last boundary is news
for the watcher once the process exits. `-b steer` to a running Session saves
and returns: there is no turn of its own to wait for.

**One process per Session.** `-b steer` holds `lock_session_process` for the
whole turn and checks `run_refusal` under it, like `continuing`, so a held
start and a foreground turn can't both launch; a concurrent `lf resume` waits
30 s, then fails as busy.

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

Done in slice 1: `--message` on `resume`, `session new` and ssh dispatch,
with its checks, callers and tests (`served_reads.rs`, `transport_tests.rs`
and the benchmark scripts' remaining `--message` are other commands' flags).
Remaining:

- `public_commands_save_steers_without_launching_providers` stays: with no
  server the commands still launch nothing; the server's half is the new
  integration test.
- Release notes under `release/` are shipped records and stay.

## Slices

1. **Foreground turn.** `lf -b steer S TEXT` on a stopped Session: save
   claimed, take the launch lock, `run_refusal`, run `run::resume` with the
   full steer text, deliver, print the reply. Graph correction switches to it
   and `--message` is deleted in the same slice, with its tests moved. Proves
   or kills: claim-at-save composes with the turn's own hooks (no second
   injection); the correction still reads its answer from the step capture;
   the AgentProcess is recorded inside the launch lock.
   *Built.* `steer_turn` (`lf/commands/session.rs`) delegates first, then
   holds the Session's launch lock for the whole turn; a running Session or
   someone else's finished Run only saves. `take_session_steers` saves the
   steer and marks every unread one delivered in one write, so hooks inside
   the turn print nothing twice (the claim-lapse question disappears).
   `steer::render_complete` carries full text. Graph correction spawns
   `--batch --output-schema S steer CONV TEXT` and its test passes reading
   the same step capture. `--message` is gone from `SessionConnectionArgs`,
   ssh dispatch, `session new` and `-b resume` (an error naming `lf -b
   steer`). `run::resume` takes the store and Session; `continues_own_step`
   and `agent_running` replace its inline checks.
   *Taught:* holding the lock to the end sidesteps "is the AgentProcess
   recorded inside the lock": a concurrent `lf resume` waits (30 s, then
   "launch is still busy"). Slice 2's held start will meet the same wait.
   *Not yet tested:* `-b steer` to a running Session saves and returns
   (needs a Live AgentProcess fixture); gate or slice 2's server tests.
2. **The server starts stopped Sessions.** The watcher's stopped case through
   `continuing`; the per-steer start column and its migration; Task steers
   naming the primary conversation (and a running worker) as wake targets;
   `lf steer`'s three-way message. Proves or kills: a `lf session resume` in a
   held terminal yields as idle without a viewer, so the existing inbox and
   app-server wakes deliver. If it doesn't, the held start passes the steer as
   the resumed conversation's first input instead.
   *Built.* Killed the idle path: a resumed Claude records a turn end (and its
   inbox) only at its Stop hook, and with no input it never takes a turn, so
   `idle_conversations` never lists it. The fallback is the resume itself:
   `resume_session_with_env` gives an interactive resumed thread every unread
   steer as its first input when one names it (`take_steers_to_start`), for
   any resumer, `lf resume` included. The per-steer column became a one-row
   cursor, `steer_wake_reading` (migration `steer_wake_reading`): the server
   reads steers past it once (`read_steer_wakes`, a rowid range), so a
   restart or the upgrade starts nothing old and nothing is stamped on
   `steers` (whose triggers move the planning revision). The watcher
   (`wake::start_stopped`, given the Registry) starts each named stopped
   Session once per reading with `continuing`, skipping one a held terminal
   already runs. `lf steer SESSION` prints one of the three readers.
   Tests: storage (`each_steer_wakes_the_session_it_names_once`,
   `a_starting_conversation_takes_its_steers_when_one_names_it`: Task steer
   → primary only), wake (`steers_start_a_stopped_session_once`: two steers
   one start, restart starts nothing), integration
   (`session_steers::a_steer_starts_a_stopped_conversation_on_the_machine_server`).
   *Taught:* a managed Codex resume (`initial_prompt_on_resume` with turn
   reports) keeps its captured first turn, so the steer is not its first
   input; it is taken only on the other path. Codex in a held terminal is
   unproven; see Open below.
3. **Docs and help.** *Done* inside write-docs and slice 2: `lf steer`'s Clap
   help names the server start and `-b`; `.lf/qa/provider-switching.md` drives
   `lf -b steer`.

4. **One rule for who a steer starts.** *Built.* `naming()` in
   `sqlite/steers.rs` is the one "unread steers naming ?1" fragment;
   `take_steers_to_start` asks it and `read_steer_wakes` keeps each
   candidate (steer, Session) past the cursor only when it holds. Test
   `a_steer_its_start_would_not_take_starts_nothing` (superseded Task steer,
   shared steer another Session read first). The candidate join no longer
   filters `update:` sources or delivery itself; `naming()` does.

Checks run 2026-11-17 (realign): `cargo test -p loopflow-storage --lib
steers` 21 passed; `cargo test -p loopflow --lib
steers_start_a_stopped_session_once` passed. CI runs the suites at gate.

5. **Launch names in refusals (steer 474316e8).** *Built.* `lf resume` of a
   headless Session says "ID is a worker (…)" or "a batch (…)"; `lf resume`
   opens only work sessions and chats (`human_session::headless_kind`, from
   `graph_lf_process_id`). `lf steer`'s reader line says "a finished worker"
   or "a finished batch". Docs: `docs/dev/execution.md`, the server README,
   `docs/dev/reference-cutover.md`. Item 2 (`is_bare` reopening a work session
   as a chat) needs LOO-924's code; reading in `scratch/questions.md`.
   Checked: `cargo test -p loopflow-execution --lib ops::human_session::tests::`
   28 passed. Gate runs `test(transport_tests)`'s worker refusal and
   `documented_commands::`.

Next: gate. For gate: the Codex question below, and `-b steer` to a running
Session (saves and returns) has no test.

## Excluded

`open` vs `resume`, selectors, `session restart`/`fork` verbs, `move` words,
`--prepare`: later slices, each designed first. Waking on `--machine` routing
runs wherever the command already runs; no new routing.

## Docs written (write-docs)

`docs/sessions.md` (Steer without interrupting, One more turn here, the bare
and crash tables), `docs/agent-api.md` (Steer), `docs/config.md`,
`docs/dev/execution.md`, `docs/dev/planning.md`, `docs/dev/reference-models.md`,
`rust/loopflow/src/server/README.md` (Starting a stopped conversation),
`.lf/qa/provider-switching.md`. `docs/sessions.md`'s table says a started
Session "reads the steer first"; for Codex that holds only if the Open
question below resolves yes.

Open: the first input rides `Launch::terminal_input`, which every harness's
`open` uses, so Pi and OpenCode take it the same way as Claude (untested).
A managed Codex resume sends its captured first turn instead
(`sends_first_turn`), so a started Codex Session hears its steers only when
its app-server yields: unproven, needs a Codex held-terminal fixture.
