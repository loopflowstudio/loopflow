# LOO-711 · Task Sessions tell you what their background work is doing

Status: implementation stopped at delivery design, 2026-11-02. Requested by Jack Heart.

## Current direction after prototype review

Jack asked to combine worker messages into the Task Session with a separate
activity view, offering a minimal default and expanded detail. This supersedes
the kickoff's wait-only proposal and its exclusion of Desktop changes below.
The kickoff implementation details remain historical proposals, not an accepted
build plan.

Jack's latest preference is screen 2, the run page in
`scratch/prototype-screens.html`: "2 is probably the closest." It groups history
by loop, with expandable steps and short summaries.

Jack subsequently requested restarting the Task worker on Codex if necessary.
Continue the existing feature Graph from explore into pursue with this direction,
reconciling the kickoff proposal before implementing. Preserve the authored demo
review; this request does not authorize shipping or merging.

Constraints from the review:

- Combine position and readiness in one representation.
- Reduce noise and repeated facts; the earlier tiles, path, readiness ladder,
  timeline and table repeated too much.
- Files owns the diff. The activity view links there rather than repeating it.
- Keep worker posts and the additional view together in the design.

Still to settle: the minimal run-page composition, which events deserve a
Session turn, the source of per-step summaries, and cross-provider delivery.
The HTML screens use simulated data; they do not establish that all displayed
per-step costs, file counts, summaries or proof are available from current APIs.
Temporal was discussed as a reference for execution history and its presentation;
Jack has not requested adopting Temporal or adding durable replay.

Source: the continued Session's Claude conversation log at
`/Users/jack/.lf/runs/f1/f15d2a56a6b84df2b6427996a923fad0/conversation.md`.

## Implementation reconciliation (2026-11-02)

Jack Heart's selected outcome remains **worker posts into the Session plus the
run page**. Neither a Desktop status badge nor a next-prompt context injection
alone meets it. The wait-only kickoff below is historical; its scope, deletion
list and Done when are not the current implementation contract.

The code inspection found no existing worker-to-native-Session posting operation.
This is a missing delivery design, not evidence that native delivery is impossible.
In particular, the kickoff's blanket statement about Codex is too strong:

- `lf/commands/util.rs::serve_codex_native` bootstraps a `CodexHarness` with
  `ApprovalPolicy::NativeClient`, closes the bootstrap connection, then runs a
  `CodexConnection` relay for the native terminal. That relay is the existing
  owner to extend for Codex delivery. A second `Harness::send_input` caller is
  not already connected to an interactive Session.
- `harness/codex_connection.rs` fences native writes and retains uncertain
  dispatch. Worker delivery needs to fit that ownership and receipt path,
  including busy turns, client reconnect and attachment transfer. Connecting a
  separate sender does not by itself prove that the native UI receives the
  turn or retains approval handling correctly.
- Claude's SendMessage probe in `research-harnesses.md` establishes a useful
  provider primitive. No production Claude session-bus transport or stable
  address mapping exists in this checkout. A new model invocation per delivery
  is not established as the product mechanism by that probe.
- `transport.rs::Request::Input` accepts bytes from the controlling viewer.
  Typing a worker message there would be input into Jack's draft. It is not a
  safe substitute for provider-level delivery.
- Steer callbacks deliver at provider boundaries; they do not supply an idle
  wake. Reusing authored steers for mechanical step exits would also duplicate
  the GraphProcess record rather than establish the missing transport.

The architecture decision still needed is the supported native delivery path:
extend each existing provider owner, including receipt and reconnect semantics,
or explicitly accept a narrower provider scope. Treating an app-owned card next
to the terminal as a post *into the Session* would change the selected outcome.
No such substitution or provider restriction is recorded as Jack's decision.

### Run page: implementation shape supported by existing data

The run page can replace the existing presentation rather than create another
activity surface. `swift/LoopflowMac/Views/TaskWorkView.swift::GraphProcessLog`
already reads every Task Graph; `WorkObservation` supplies live local details and
`RegistryQuery.taskGraphProcesses` supplies explicitly refreshed remote details.
The latter is not a live subscription, so automatic remote updates remain work.

- Minimal: the Task's current execution/position in one line, the latest run,
  and a Files action. The Task execution projection owns readiness; a completed
  Graph process does not prove that the Task reached `end` or passed review.
- Expanded: newest runs first, then contiguous groups of recorded loop
  positions, then expandable steps. Use the whole nested `iterations` value;
  one loop counter would mislabel nested loops and repeated nodes.
- Each step initially shows its recorded label, duration and process exit.
  `GraphStepProcess` has no prose summary, cost, changed-file count or proof
  field. Those prototype values must not be fabricated. A later prose summary
  must come from the Session turn captured by that exact step Process and
  remain distinguishable from its mechanical result.
- Files retains the diff. Existing Task navigation, run controls, remote
  location selection and Session surfaces retain their current owners.

### Delete — do not maintain (current target)

- Replace `GraphProcessView.detail`'s default diagram plus flat step list with
  grouped run history. Preserve every recorded step, nested loop position,
  process identity, timestamps and exit evidence. Keep `GraphDiagram` for the
  graph-definition surface; it is not itself obsolete.
- Replace the run header's bare `graph.state.rawValue` where current Task
  readiness is intended. Preserve historical run outcomes independently of
  current Task state; do not relabel all completed runs as Task completion.
- Replace the task-session skill's between-turn contract only when the chosen
  native delivery path exists. No instruction may promise a wake from a
  next-prompt steer.
- Do not delete `ops::task::task_wait` or change its output as part of the
  current target merely because the superseded kickoff proposed it.

### Remaining acceptance

1. Native delivery and its receipt/reconnect semantics for the selected provider
   scope. Idle, busy, draft-present, detached, takeover and unknown-delivery
   cases must preserve the same Session and avoid replaying uncertain input.
2. Worker updates read existing Graph/Process facts, exclude worker Runs as
   recipients, coalesce a step exit and next start, and never imply permission
   to advance an interactive review or launch another Graph.
3. The run page and Session posts agree on those same facts; nested loops and
   failed/stopped runs remain inspectable; missing summaries stay missing.
4. Focused provider/relay tests and headless production-view tests, followed by
   the authored feature Graph demo. Publication and merge remain outside this
   request.

No production changes were made in this implementation pass. Building the view
alone would leave the selected outcome incomplete; applying the old wait-only
plan would implement the superseded alternative. The missing native delivery
contract returns this design to review before dependent implementation.

Check: source inspection only; no production diff, so no build/test run. Native delivery acceptance remains unproved.

## Kickoff proposal (superseded where noted above)

## Problem

Jack leaves a Task Session open while `lf task run` works in the background.
The Session hears nothing: it acts only during a turn, and no turn starts when
a step ends, fails, or stops at an interactive step. So Jack types "what's
going on now? did it finish?" to get a report he should already have.

Outcome: Jack can leave a Task Session open and learn when its background work
moves, finishes or fails, without asking.

## Demo

In Desktop (or a terminal), `lf resume --task LOO-711`. The Session reads the
Task, says where it stands, and ends its turn. In another terminal,
`lf task run LOO-711`. With nothing typed, the Session speaks:

> kickoff started (Graph `feature`, Process b40d…).

and later:

> kickoff finished; explore is interactive and waiting here. Read
> `scratch/loo-711-task-sessions-tell.md` and tell me which approach to build.

If the step fails, the Session says so with the log path. The cursor stays in
Jack's input box throughout; a half-typed draft survives.

## What exists (findings, 2026-11-02)

Verified in this checkout. Paths are under `rust/loopflow/src/`.

- **An interactive Task Session is Claude Code's native TUI on a pty** held by
  a detached owner process (`transport.rs`); Desktop attaches a Ghostty pane as
  the controlling viewer. Only that viewer's token may write to the pty
  (`transport.rs:614-624`). lf has no `session prompt`; `lf desktop send` types
  into the pane like a person and would collide with a draft.
- **Steers are pulled, never pushed.** `lf session steer` saves a row and the
  hooks lf installs (`context_block.rs:242-265`: SessionStart, UserPromptSubmit,
  steer-gated PostToolUse, Stop) paste pending steers into context at the next
  boundary. A steer saved while the Session is idle appears on Jack's next
  prompt. Stop never blocks or continues a turn. Nothing wakes an idle Session.
  The comment at `steer.rs:10-14` ("Prompt boundaries always check") is stale
  since 3ce34e29c8c; both prompt and tool moments use the seen-marker check.
- **Claude Code re-invokes a Session when a background command exits.** Probed
  here: a `run_in_background` Bash that exited after four seconds produced a
  task notification and a new model turn with no person present. Third-party
  write-ups describe the same for interactive sessions (the harness sends one
  completion notification, "like SIGCHLD"). This is the only mechanism that
  starts a turn in the native TUI, and it is provider-native: no lf process
  writes to the pty.
- **`lf task status <issue> --json --watch` already streams a frame on every
  change** (`lf/commands/watch.rs`): store revisions say when a commit could
  have changed the result, a kqueue on the WAL wakes the reader, a 2 s clock
  covers Process liveness. The `task` body carries `task.state`, `task.reason`
  (a sentence such as "Step Process … is running kickoff"), `task.graph.step`,
  `graph_processes[].{state,step}` and `task.pr`. The `graphs` and `processes`
  revision domains cover `task_graphs` and Graph step Processes.
- **`lf task wait`** (`ops/task.rs:2558`) is a 1 s poll of the full status,
  with `--until submitted|terminal` only, and returns success on timeout.
- **Step lifecycle, as recorded.** `RunningGraph::spawn` (`lf/commands/
  graph.rs:791-884`) starts each step as an `lf` child, writes its
  `graph_process_steps` row once the child registers (`graphs` domain), and
  `child.wait()`s; the exit lands on the step's own `processes` row
  (`processes` domain). A run that stops before an interactive step, or
  reaches `end`, arrives in `task_graphs` (`planning` domain); a failed or
  interrupted run writes nothing there. So one step boundary is two commits a
  process spawn apart, with `task.reason` reading "Graph is between steps
  after kickoff" in the gap. `graph_processes[].state` reports both "reached
  end" and "stopped at an interactive step" as completed; `task.graph.step`
  and `task.reason` tell them apart.
- **Nothing announces steps today.** No per-step comment, Linear comment or
  Task event; `TaskEventKind::GraphFinished` has no live writer. Task steers
  reconcile to every member Session, including the Graph's own step Runs, so
  a step-event steer would be delivered back to the worker that caused it.
- Wave memory, 2026-10-01 (Ask removal): Jack asked for no notification queue,
  reporting command or required handoff. Steers are the one durable authored
  input; there is no agent messaging substrate. Step events are already Work
  state (GraphProcess rows), so no new record is needed.

## Decision (kickoff, 2026-11-02)

**The Task Session subscribes itself, through the provider's own background
notification, to an lf wait that returns when the Task's work moves.**

Two changes, no new store, no new delivery channel:

1. `lf task wait <issue> --until moved` returns when the Task's work position
   differs from when the wait began. It is driven by the watch machinery, not
   a poll; `submitted` and `terminal` move onto the same loop.
2. The task-session skill arms that wait in the background before ending any
   turn in which the Task has live work, and treats its return as a turn:
   reread status, say what moved and what is available now, re-arm.

Why this and not the other options Jack named:

- *Better skill instructions alone* cannot reach an idle Session; the brief
  already saw this. They are half of the answer: the arming rule.
- *Automatic injection from the worker Graph into the Session* (a steer or a
  context-block line per step) only lands on Jack's next prompt. It would add
  a second record of what GraphProcess rows already say, and the 2026-10-01
  decision rules out a notification queue. The wait reads Work state instead.
- *Graphs providing their own updates* is a prose channel beside the step
  Processes that already carry the facts. Not needed for this outcome.
- *A Stop hook that blocks and continues the turn* would hold the Session busy
  for the length of a step (pursue runs for an hour), make it read as
  unavailable, and keep Ctrl-C semantics undecided. Rejected.
- *A Desktop badge* lets Jack see movement without typing but does not make
  the Session say anything, and leaves terminal Sessions out. Free to add
  later; not this Task.

### Which events start a turn

Every move of the Task's work position: a step Process starts or ends, the run
stops before an interactive step, the Graph reaches `end` or fails, Auto-Ship
state changes, the PR opens, merges or closes, the Task completes. Not: token
usage, comments, Session liveness, or a Linear sync that changes nothing in the
position. A step boundary is one event: the wait's settle window (below)
folds the step's exit and the next step's start together.

Rationale: Jack's complaint is silence. A turn per step boundary is one short
read and two sentences; the `feature` graph has about ten steps per pass. If
this proves noisy, the knob is the skill text ("report only failures, waits
and end"), not the command. Recorded as a product assumption in
`scratch/questions.md`.

### Interactive and headless

Only an interactive conversation arms the wait. A headless `lf --task <issue>
skill task-session` run ends when its turn ends; a background wait would keep
it alive for nothing. The run-mode line in the launch context says which it is.

Codex, OpenCode and Pi Sessions have no provider notification that starts a
turn; for them the wait returns at their next prompt. Tasks run on Claude, so
this is a documented limit, not a gap to close here.

## `lf task wait --until moved`

Source of truth: the `lf task status --json` body (`task_detail`), read by the
same reader `--watch` uses. The wait compares a **position** projection of two
readings and returns when it differs:

```
task.state, task.reason, task.graph, task.pr, task.next,
graph_processes[].{id, state, step}, task done/abandoned
```

Everything else in the body (comments, sessions, timestamps) is ignored.

Behaviour:

- `--until moved` (new): returns on the first position change after the wait
  began, once the position has held still for a settle window of 2 s (capped
  at 10 s after the first change). The window folds a step's exit and the
  next step's start into one return, so "between steps after kickoff" is
  rarely what the Session reports. `submitted` and `terminal` keep their
  predicates and run on the same loop, without the window.
- Output: what `lf task status --json` prints, with or without `--json` as
  today's status does. The old `TaskSnapshot` shape goes away from wait.
- Exit status: 0 when the condition was reached; 2 when `--timeout` elapsed
  first (status is still printed). `after_help` states both.
- Mechanism: `watch::start` with `Transport::Pull`, a `Watched::every
  (ACTIVITY_CLOCK)` reader (process liveness is outside the store), and the
  caller pulling frames until the predicate holds. The 1 s poll in
  `task_wait` is deleted.
- No LM is involved; a wait is one lf process reading on store revisions.

## Skill change

`rust/loopflow/src/builtins/task/skill/task_session.md`, replacing "What
happens between turns":

```
## Hearing the Task move

A turn starts when the person writes, or when a background command you started
returns. That return is how you hear the Task: no steer or hook wakes an idle
conversation.

- Before ending any turn in which the Task has a live Graph process, a run
  waiting on checks or an open PR, start `lf task wait <issue> --until moved
  --json` in the background: one command that exits on the first change, not a
  monitor. It returns when a step starts or ends, the run stops at an
  interactive step, the Graph reaches `end` or fails, or the PR opens, merges
  or closes. Exit 2 means its timeout passed with no change.
- When it returns, that is your turn. Reread status, say in a sentence or two
  what moved and what is available now (the step waiting here, a PR to
  review, a failure with its log), then apply the procedure and arm the wait
  again while work is live. Say nothing when nothing moved.
- Headless, do not arm: report and exit. Never hold a turn on a foreground
  wait unless the person asked to wait now. Do not poll.
```

The same "Within a turn, `lf task wait …`" sentence in `wave_session.md` and
`repo_session.md` is corrected to the new flags; those conversations operate
many Tasks and keep their current turn model (exclusion below).

## Risk findings

- **Does the native TUI start a turn on a background notification while
  idle?** Confirmed headless by probe; interactive is documented by third
  parties, not by Anthropic. The demo's first check. If interactive Claude only
  queues the notification until the next prompt, the design degrades to
  "appears on your next turn" with no code change, and the fallback wake is
  `lf desktop send` keyed by Session plus `send-key enter` (Desktop only,
  draft-colliding). Decide then, not now.
- **A wake can be lost mid-turn.** anthropics/claude-code#86365 reports a
  background notification dropped, not queued, when it arrives while the
  Session is mid-turn; closed "not planned" 2026-10-16. The skill's rule that
  every turn starts from fresh reads and re-arms before ending makes a lost
  wake cost one report, delivered on the next turn. The interactive wake
  itself is confirmed: probed in this Session on 2026-11-02 (13:33) and by
  #87689's reporter. See `scratch/research-harnesses.md`.
- **Compliance.** The arming rule is skill text; a model that forgets to re-arm
  goes quiet until the next prompt, which is today's behaviour. Mitigation is
  the rule's placement (a turn-ending check, like the existing "end a turn
  only when the Task has its disposition") and the demo.
- **Background limits.** Claude stops a background command after at most two
  hours and re-invokes; the Session re-arms. `pursue` loops longer than that
  cost one extra wake per two hours.
- **Store availability.** The wait needs this Machine's store; without one it
  fails as `--watch` does ("only a registered Task can be watched").

## Scope

User-visible outcome: with a Task Session open and idle, running its Graph
makes the Session report each move with no message from Jack.

Affected surfaces: `lf task wait` (flags, output shape, exit status),
task-session skill, the two sibling session skills' wait sentence,
`docs/work.md` and `docs/agent-api.md` wait examples, `docs/lf-reference.md`
(synced from Clap). Desktop and wire DTOs unchanged.

Absent and error states: no Task for the issue is an error as today; a Task
with no Graph process and no PR returns only when planning closes it; timeout
exits 2 with the current status.

Exclusions: Wave and repo Sessions keep their turn model; a Desktop "moved"
badge; a UserPromptSubmit context line summarising moves since the last turn
(the lazy path, revisit only if compliance evidence calls for it); Codex and
OpenCode wake-ups; the writerless `TaskEventKind::GraphFinished`, an earlier
notion of a finish event kept for reading old rows, whose removal is its own
cleanup.

### Delete — do not maintain

- The 1 s polling loop in `ops::task::task_wait` (replaced by the watch loop).
- `print_task` / `TaskSnapshot` as wait's output (wait prints the status body).
- "You act only during a turn; nothing schedules your next one. Do not promise
  to watch, check back or follow up later." in `task_session.md`.
- The stale comment at `steer.rs:10-14`.

Forbidden: a step-event table or steer kind; a second wait command; a Stop hook
that continues turns; any lf write into a Session's pty.

## Done when

- `lf task wait <issue> --until moved --json` in one process returns, with
  exit 0 and the new status body, within five seconds of another process
  recording a Graph step Process for that Task, and returns once when that
  Process's exit and the next step's row are written 500 ms apart;
  `--timeout 0s` exits 2 and still prints the body. Integration test in
  `rust/loopflow/tests/` (the standalone wait assertion at
  `task_initialization_tests.rs:350` moves to the new shape).
- The three session skills name the new flags; `cargo test -p loopflow --test
  integration documented_commands::` passes.
- Demo: the moment described above, in Desktop, with nothing typed.

Check: design only, no build or test run this step.
