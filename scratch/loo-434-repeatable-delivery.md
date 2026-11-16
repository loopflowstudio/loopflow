# LOO-434: repeatable delivery you can approach from the Task workspace

Status: explore, round 1. Three proposals in [explore.html](explore.html); no
implementation direction chosen yet. The technical plan below is the kickoff
candidate, not a settled design.

## Explore: what should organize the experience? (2026-11-08)

**A. Follow the route.** The captured workflow is the main view. Review stops,
execution, iterations and delivery all sit on one route; select a stop to inspect
what happened and what continuing does. Closest to the kickoff plan and smallest
change. Its cost is asking a newcomer to read the route before doing useful work.

**B. Work the next decision.** A compact action area accompanies the current review
page or conversation. It says what finished, what needs a response and what the
next move runs. The whole route and run history open on demand. Same shared Rust
moves as A; the main difference is progressive disclosure and keeping the review
in view. Risk: hiding the route can hide the larger plan, so the next stop and
merge effect must stay visible. Recommended starting point.

**C. Start from a delivery that worked.** A past verified delivery becomes the
entry point for the next Task: inspect its checks and recorded instructions,
compare the current definition, then start a new Task from the current workflow
or prepare an editable improvement. The aspiration is a library of useful,
repeatable deliveries without a second template engine. Largest scope: linking
verification, exact instructions and definition changes well enough to compare
them. A successful old run proves nothing about a new Task's checks or approval.

All three keep the same Task conversation, existing Graph engine, explicit moves,
Stop and honest failure recovery. A and B still support reuse through history;
C makes reuse the starting experience. All expose merging before the move.
The page uses fictional example runs; it is not a native Desktop acceptance test.

LOO-733's local design was read on November 8. Its settled direction is a built-in
interactive review panel, off the Task thread, with buttons and one participant.
Its round-3 alternatives remain open. This Task does not choose that panel's
layout or implement Send/round storage. Its move projection is shared work;
ownership must be reconciled before either Task implements it.

Open review: **1a** route, **1b** next decision, **1c** past delivery, or a combination.
**2a** taskless terminal Continue requires `c`; **2b** Enter continues. Recommendation:
2a, so leaving a provider and an extra Return cannot accidentally advance.
Storage shape and bug/maintenance entry-point curation stay implementation/design
work, rather than five unrelated decisions required before exploring.

Fresh observation: Graph run `c070ec5d-0e3b-41c2-84c9-1a75b9386c82` completed
kickoff (`046ff13c-777c-4557-9b1e-be75c97f3838` succeeded); Task stands at explore.
PR #1851 is open with planning files only. The current Session remains
`01f484c818004d53b393cd8d59c2d090` after changing providers.

## Problem

Someone opening a Task in Desktop should be able to pick what they want done
(fix a bug, build a feature, answer review feedback, keep the repository healthy),
see what that will ask of them and check, start it, follow it in plain words,
stop or redirect it at a boundary, and later reuse or improve it, all without
learning Graph, Flow or Process vocabulary first. The original LOO-434 requirement
still holds: in a two-step interactive Flow, Continue advances exactly once, Stop
leaves later steps unstarted, and the last step visibly finishes. Original
evidence: Jack found Ctrl-C-based advancement awkward during LOO-421; PR #1500
restored interactive execution without an understandable Continue/Stop.

## What exists today (read 2026-11-07, lf 0.15.1)

**Engine (Rust), sound and reused unchanged:**
- A Task captures one Graph on first `lf task run` (`task_graphs`, full compiled
  graph *with skill text*, ~65 KB for `feature`). `TaskGraph` on the wire:
  name, interactive steps (name, skill, first prose line), position
  `at{step}` | `run{step, lf_process_id, running}` (`rust/loopflow/src/task_graph.rs:332`).
- `lf task run` stops before the next interactive step; the Task then waits "at"
  it in its conversation. `--from <step>`, `lf ship`, `lf task interrupt` exist.
- Each Graph process appends `graph_processes` (graph name + compiled skeleton)
  and `graph_process_steps`; step results come from Process exits.

**Gaps found:**
1. **Past runs don't keep their instructions.** `graph_processes.compiled` holds
   labels only (checked: no `content` in this Task's live row). Each step gets its
   skill through a `tempfile::NamedTempFile` (`lf/commands/graph.rs:1008`), deleted
   after the step. `task_graphs` keeps the text but is replaced on take-up. Only the
   provider transcript of each step Session still has what the agent was told.
2. **The live Task conversation isn't told how to work a review step.** At launch it
   gets `TaskGraph::guidance` ("works this step with the explore skill; when the
   person is done, `lf task run` continues"). When a run later *arrives* at an
   interactive step, the hook update (`harness/task_updates.rs:position_text`) only
   says "At explore · available for review … not a request". The same conversation
   doesn't learn which skill to work or what Continue does.
3. **Taskless in-place Continue/Stop is exit-code driven.** `lf graph X` runs an
   interactive step in the terminal; exit 0 advances, 3/130 (Ctrl-C, takeover)
   holds (`lf/commands/graph.rs:863`). Continue vs Stop is how you quit the
   provider. That is the LOO-421 awkwardness, still installed.
4. **Swift derives the workflow itself.** `WorkflowDefinition.init(CompiledGraph)`
   (`swift/Loopflow/Models/Graph.swift:190`) recomputes columns, runs and back-edges
   from compiled nodes: a second authority for "what Continue crosses".
5. **Three position readouts, two diagrams, no live step view.**
   `TaskWorkflowHeader` label (`TaskWorkView.swift:222`), `TaskWorkflowBar` live
   line (:420) and `GraphProcessLog`'s `TaskRunPosition` (:1033) each phrase position.
   `WorkflowDiagram` (interactive steps only) and `GraphDiagram` (full steps, can
   colour from a `GraphProcessDetail`) both exist; `GraphDiagram` is only ever called
   with `latest: nil`.
6. **No Stop for a running run in Desktop.** Only Stop Auto-Ship. `lf task interrupt`
   exists in the CLI.
7. **Button labels don't say what they cross.** "Continue" at `demo` gates and
   *merges* a PR before stopping at `verify`.
8. **New Task offers Graph names, not outcomes.** `NewTaskSheet.WorkflowChooser` is a
   searchable catalogue with the default first.

LOO-733 (active, round 3 at `explore`) independently found gaps 4/7 and the missing
back-edges in `TaskGraph`, and designs the review panel (rounds) plus
"moves derive from the Graph". See `../loopflow.loo-733-decide-how-people/scratch/loo-733-task-rounds.md`.

## Ownership split with neighbours

| Concern | Owner |
|---|---|
| Rounds at a review step: page, tray, Send, `lf task respond` | LOO-733 |
| Moves on the wire (what each Continue/Back/Ship crosses), Stop, the Workflow view, Continue/Stop proof | **LOO-434** (this) |
| Retry from the failed step | LOO-435; this shows today's truthful recovery |
| Opening the primary conversation, its retention | LOO-426, LOO-416, LOO-791 |
| Step diagram sizing inside a pane | LOO-392 |
| Composed real-provider proof | LOO-391 |
| Provider-limit recovery | LOO-747; this reports it |

LOO-733's panel consumes `TaskGraph.moves` from this Task. If LOO-733 lands moves
first, this Task consumes theirs instead; there is one implementation either way.

## Kickoff candidate

### 1. Rust owns moves; every surface renders them

Add `moves` to `TaskGraph` (wire DTO, fixtures in all three languages):

```json
"moves": [
  {"kind": "continue", "from": 1, "runs_first": "write-docs",
   "stops_at": "demo", "merges": false, "reaches_end": false},
  {"kind": "back", "from": 0, "runs_first": "kickoff", "stops_at": "explore",
   "merges": false, "reaches_end": false},
  {"kind": "ship", "from": null, "runs_first": null, "stops_at": "end",
   "merges": true, "reaches_end": true}
]
```

- `from` is the compiled node key; `lf task run --from '#<key>'` already accepts it,
  so a move names its exact Place (fixes `CapturedGraph::find` returning the first
  same-named step).
- `merges` and `reaches_end` come from existing `after`, `next_interactive` and
  `landing_step`. A `stopped` run gets `{"kind":"again", "from": <run start>}` saying
  truthfully that completed steps repeat (LOO-435 changes that later).
- Running runs get `{"kind":"stop"}` → `lf task interrupt <issue>`.
- Swift renders labels from moves with one phrase function:
  "Continue: builds, publishes a PR, stops at demo". "Continue: gates and **merges**
  #1850, stops at verify". Delete `WorkflowDefinition`'s run derivation.

### 2. One Workflow view in the Task page

Replace the header label, the bar's live line, `TaskRunPosition`, the Overview
`WorkflowDiagram` and the `WorkflowCatalogInspector` with:

- **Now line** (one function, used by the Task page and the compact bar): what is
  true, in this order and never merged: *where the Task stands* (Workflow position),
  *what is running* (Process liveness: running / stopped with exit / unknown), *Task
  state* (open, done). Failed, stopped, unavailable and completed each get distinct
  text and colour (Error red only for a failed exit; stopped-by-you is neutral).
- **Next**: the moves as buttons, each saying what it crosses. At a review step the
  primary action is "Review here" (focuses the Task conversation; LOO-733's panel
  when it exists); Continue sits beside it, never default-focused.
- **Steps**: `GraphDiagram` fed the latest `GraphProcessDetail`, interactive steps
  emphasised, iterations as pass counts. Sizing stays LOO-392's.
- **Runs**: the existing `GraphProcessView` history, each step row gaining
  "Instructions used" (§4) and the PR it published or landed when recorded.

`TaskWorkflowBar` keeps its place above the pane but renders the Now line and the
first move only; its dot stages go.

### 3. The main conversation drives review steps

- When a run arrives at an interactive step, the task-update carries the same text as
  launch: `TaskGraph::guidance` (skill to work, what `lf task run` continues to).
  One function, two delivery times. Still observation: it starts no work.
- Moves from Desktop run the same argv the conversation would (`lf task run …`,
  `--reason` carrying any typed words). The conversation sees the move as a
  task-update. No review Session, no self-opening instruction.
- Taskless `lf graph`: after an in-place interactive step's provider exits 0, with a
  terminal and a later step, lf asks once:

  ```text
  explore is done.
  ⏎  Continue: write-docs → … → pr-publish, stops at demo
  s  Stop here (nothing after explore starts)
  ```

  Stop exits 3 (held), leaving later steps unstarted. Non-TTY keeps today's
  behaviour. Ctrl-C/Esc inside the provider stay the provider's: they interrupt a
  turn, never advance. This is the cmux/herdr Continue/Stop demonstration, since those
  hosts run `lf graph` in a pane.

### 4. Past runs keep the instructions they ran

Additive migration (`scripts/new_migration.py run_instructions`):

```sql
CREATE TABLE skill_texts (sha256 TEXT PRIMARY KEY, text TEXT NOT NULL) STRICT;
```

The Graph process inserts each skill occurrence's `source_text()` (insert-or-ignore,
content-addressed) and records `source_sha256` per skill node in the compiled
skeleton it already appends. `lf graph show <run> --step <key> --instructions` prints
it; Desktop's "Instructions used" reads the same. Editing today's skill changes the
next take-up, never a past run. Older runs without a hash say "not recorded".

### 5. Outcomes at New Task

The chooser leads with outcomes, each a named existing Graph with one line on
what you'll be asked and what gets checked, read from the Graph's header comment:

| Outcome | Graph | You take part at |
|---|---|---|
| Build a feature | `feature` | explore, demo, verify |
| Fix a bug | `code` | demo, verify |
| Answer review feedback | the Task's Back move to `implement` with `--reason` | demo |
| Keep the repository healthy | open (questions.md) | |

"Answer review feedback" is a move on an existing Task with a PR, not a New Task
entry. The full catalogue stays one click away. Improving a workflow stays
`lf graph customize` / `lf skill customize` committed in a Task's PR; Desktop's
existing Customize/Edit opens the file.

## Absent and error states

| Consumer | Absent / failed means |
|---|---|
| Now line, no Graph taken up | "Not started · runs `code` (repository default)" |
| Liveness unknown (Process record but no OS evidence) | "Unknown whether still running"; Stop stays offered |
| Store/stream unavailable | last-good content, moves disabled with reason |
| `moves` empty at `end` | "Finished · completion requested" or "Done" |
| Instructions not recorded | "Not recorded (run predates this)" |

## Forbidden

- Swift computing what a move crosses, or a second position phrase.
- A move whose label hides merging or reaching `end`. The existing Graph can merge
  during ordinary Continue after demo, and reaching end has shipping/completion
  effects; expose those effects rather than promising Continue never approves them.
- A new review Session per step, or a move inferred from a quiet or closed provider.
- Two diagrams for one workflow.

## Delete, do not maintain

- `WorkflowDefinition` run/back-edge derivation and `WorkflowCatalogEntry`
  (`Graph.swift:187-266`) and `WorkflowDiagram.swift`, with their exclusive tests;
  consumers move to `TaskGraph.moves` + `GraphDiagram`.
- `WorkflowCatalogInspector` (`WorkPalette.swift:394`) and the `.workflow(name)`
  palette destination; `.graph(name)` remains.
- `TaskWorkflowBar.stages()` dots and its live line; `TaskRunPosition`;
  `TaskWorkflowHeader.runAction` label switch.
- `position_text` in `task_updates.rs` once guidance replaces it.

## Slices

1. **This slice — moves and Stop in Rust.** `TaskGraph.moves`, `again`/`stop`, fixtures
   (`tests/fixtures/dto/task_work.json`, `task_overview.json`, Swift `DTOFixtureTests`).
   Focused: `cargo test -p loopflow --lib task_graph::`.
2. Arrival guidance in task-updates; delete `position_text`.
3. Taskless Continue/Stop prompt; PTY integration test of a two-step interactive graph.
4. `skill_texts` + `--instructions`.
5. Desktop: Now line, moves, live `GraphDiagram`, Stop, Instructions used; deletions above.
6. New Task outcomes.

## Checks

- Explore page: `node /tmp/loo434-explore-check.cjs` passed simulated moves,
  stop/failure/recovery, reuse, terminal choice and reply composition; no JS errors
  or horizontal overflow at 1100/390 px. Light/dark and narrow viewport renders
  inspected. Default Playwright browser was absent; installed Chrome ran the check.
  These check the prototype only; native/provider acceptance remains for demo.
- `cargo test -p loopflow --lib task_graph::` — moves for `feature` at start/explore/
  demo/verify/end and a stopped run; demo's Continue has `merges: true`.
- Rust integration (`tests/integration.rs`): two-step interactive graph under a PTY
  stand-in provider: ⏎ advances exactly once, `s` exits 3 with step 2 never started,
  final step's Process completes and the Graph is `completed`. Stand-in, not native.
- Rust: Task at `explore` via `lf task run`; primary Session's next task-update names
  the `explore` skill and `lf task run`; no Process starts.
- Rust: run a graph, edit the skill file, `lf graph show <run> --step <k> --instructions`
  returns the original text.
- Swift view tests (`DesktopHeadlessTests`): Now line distinguishes failed / stopped /
  unknown / completed; move labels from fixture moves; Stop shown only while running.
- Gate: `uv run python scripts/test.py --reuse-passing`.
- Native Desktop with real configured work (start, observe, review in place, iterate,
  stop, failed step, reopen same conversation, finish): demo step with a person;
  LOO-391 owns the composed proof.

## The demo

In Desktop on a real Loopflow Task: New Task → "Build a feature" shows "you'll be asked
at explore, demo, verify". Start. The bar reads "Running kickoff". It arrives at
explore; the same conversation says it is working explore and what Continue does.
Answer there. Press "Back: re-plan", then "Continue: builds, publishes a PR, stops at
demo". Press Stop mid-implement: "Stopped by you at implement · write-docs and
implement done" and Run again says which steps repeat. A `qa` failure shows red with
its log. Reopen the Task: same conversation. Continue at demo says it merges.
In a terminal: `lf graph two-step` → ⏎ / `s`. Open a past run's step: the text it ran.

## Done when

A person in Desktop can start a Task by outcome, read in one line where it stands and
what is running, see what each next action crosses before pressing it, stop a run,
review in the same conversation, and open a past run's step to read the instructions
it actually ran; and `lf graph` Continue/Stop works by keystroke, not by how the
provider was quit.
