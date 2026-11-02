# LOO-711 · Task Sessions tell you what their background work is doing

Status: implementation stopped at delivery design, 2026-11-02. Requested by Jack Heart.

## Current direction after prototype review

Jack asked to combine worker messages into the Task Session with a separate
activity view, offering a minimal default and expanded detail. This supersedes
the kickoff's wait-only proposal and its exclusion of Desktop changes.

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
alone meets it. The archived kickoff is not the implementation contract.

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

### Delete — do not maintain

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

## Retained evidence and superseded proposal

The original notes and all five HTML explorations are preserved at
`5f658180d9:scratch/`. The wait-only proposal is in that commit's
`loo-711-task-sessions-tell.md`, under **Kickoff proposal**. It proposed a
watch-driven `task wait --until moved`, new output/timeout semantics and
skill-owned re-arming. None was implemented or remains required by the selected
design. Its provider exclusions and draft-colliding Desktop fallback must not
be carried forward. The HTML files remain simulated review references.

Reusable findings from that proposal (paths under `rust/loopflow/src/`):

- `lf/commands/watch.rs` already streams Task status on store revisions and a
  liveness clock. Graph/Process rows supply movement facts without an additional
  event store. `ops/task.rs::task_wait` still polls for submitted/terminal.
- `lf/commands/graph.rs::RunningGraph::spawn` records step start and exit in
  separate commits. Coalesce exit/next-start updates rather than announcing the
  transient between-steps state. A completed Graph may mean an interactive stop;
  use the Task's position to distinguish that from reaching `end`.
- Task steers fan out to worker Runs as well as interactive Sessions. Delivery
  must select interactive recipients instead of feeding events back to workers.
- Product memory's October 1 Ask-removal decision rejects a separate notification
  queue or required handoff. Existing Graph facts remain the status authority.
- Claude wake probes and their limitations are retained in
  `research-harnesses.md`. They establish primitives, not production delivery.

Demo remains: leave a Task Session idle in Desktop or a terminal with an unsent
draft, run its Graph, and observe a report without typing. Show failure/log
navigation and an interactive stop without advancing it; inspect the same run
in the page. The draft and Session identity must survive.

Check: `git diff --check` and `lf context` pass; notes only, no build/test needed. Native delivery acceptance remains unproved and belongs to implementation/demo.
