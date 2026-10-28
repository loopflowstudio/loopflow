# LOO-627 New Task: one step from a request to its conversation and running workflow

Status: accepted (explore, 2026-10-28). Jack Heart filed this on 2026-10-26 and asked
to launch it on 2026-10-27. At explore he compared three clickable prototypes
(`scratch/new-task-prototypes/index.html`: A sheet, B talk-first, C one-line palette)
and chose **A, the sheet**, as designed below. The prototype's tab A is the visual
reference for `NewTaskSheet`. The other assumptions in `scratch/questions.md`
(`create --run`, ⌘N + menu + sidebar row) stand. On 2026-10-28 Jack also compared
three placements (`scratch/new-task-prototypes/placements.html`: top of the Task list,
title bar, a ＋ on each Wave row) on a fresh repository and a busy one, and chose
**the top of the Task list**: a permanent ＋ New Task row under the repository and
selected-Wave rows, defaulting the sheet to the scope that list shows. No title-bar
button and no per-Wave ＋.

**Sheet layout and finish (Jack, 2026-10-28, demo).** After pressing the first build
in the dev app, Jack compared three layouts (`sheet-layouts.html`: A Linear, B
composer, C preview) and three finishes (`sheet-polish.html`: Paper, Score,
Instrument) and chose **layout A+B, finish Paper+Instrument**. The reference is
`scratch/new-task-prototypes/sheet-final.html`:
- No small all-caps label anywhere (Jack, 2026-10-28: "a bit of a slopcannon tell").
  The heading is **What do you want?** (Jack chose it, 2026-10-28) in Cormorant
  italic, accent ink. The placeholder wording is open (switcher in the mock).
- One raised composer box (inset, accent focus ring) holding the request
  (placeholder "Describe it the way you'd say it to a teammate") with a tick strip of
  the workflow's steps along its bottom edge, the ones that need you filled with the
  accent. Not a segmented control: Jack noted pills don't scale to many workflows.
- Under the box, one line ties the workflow to where the Task lives: "In **myapp ▾**,
  running **code ▾** · myapp's default". A different choice reads "running **auto**
  instead of product's **feature** · Use feature". The workflow menu searches the
  whole catalog (Recent / Built in / This repository) and opens with "New Tasks in
  product run feature. Pick another for this Task only."; after a departure it offers
  "Make auto the default for every new product Task" (`lf --wave product set graph
  auto`). Open: whether that means your value (`lf set`) or the Wave's committed
  config. Then the stops line in Cormorant italic.
- Under the box (Jack, 2026-10-28), two lines: the selector on its own line, "In
  **product ▾**, running **feature ▾** · product's default" (a departure reads "· not
  feature ↺"); below it, the workflow as a chart: a rail with one dot per step,
  labelled only where you take part (explore, demo, verify in Cormorant italic,
  accent dots). No command line, no "You'll step in…" sentence. Footer: Cancel and
  a lit pill **Start ⌘↩** (was Create). Sheet: 16pt radius, soft spring entrance.
- Start closes the sheet and the new Task is selected with its conversation open.
  Jack observed in the first build that it was not: he selected it by hand. That is
  a bug being fixed, not a design change.

## Problem

Starting a Task takes three separate commands: `lf task create`, `lf task run` and
`lf resume --task`. You have to know all three, run them in order, and type the
request into the conversation a second time. Desktop has no New Task control. It has
New Wave (sidebar hover row), a Task's Start/Continue controls (#1733), and
"New conversation" on an existing Task. This affects anyone starting work in
Loopflow, especially people whose first action is "new Task".

## Demo

Desktop, `loopflow` repository, Wave `product` selected in the sidebar:

1. Press ⌘N (or click **New Task** at the top of the sidebar's Task list).
2. A sheet opens with a text field already focused: *"What do you want?"*. Type
   "lf task status prints the graph name on its first line".
3. Below the field: **Wave** `product` (the sidebar scope; can be changed to
   the repository) and **Workflow** `code · repository default`. The line underneath
   reads "stops for you at demo, verify". Changing Workflow to `auto` changes that
   line to "runs to a merged PR without stopping".
4. Press ⌘↩. The sheet closes. The new Task is selected in the sidebar and its
   conversation pane opens. The first turn is your request, word for word. The
   agent's reply says that Graph `code` is running `implement`, what follows it, and
   that it will stop at `demo` for you. The Task header shows "running from start".

Terminal, same repository:

```bash
lf task create "lf task status prints the graph name on its first line" --run
```

This prints `<title> · LOO-…` and `Graph code: running from start`, then opens the
same conversation in this terminal with the request as its first turn.
`lf -b task create "…" --run --graph auto` runs the request through to a merged PR
without opening a conversation.

## Chosen approach

`lf task create` gets one optional mode that creates the Task, starts it and opens
it. Desktop's New Task sheet calls that same command and then uses its existing
"open a Task's conversation" path. Desktop has no create/run/connect sequence of
its own.

### CLI: `lf task create [REQUEST] [--run] [--graph NAME]`

- `REQUEST` (positional) holds your words. They become the Task's notes verbatim,
  and the first line becomes the title unless `--title` is given. `REQUEST`
  conflicts with `--notes` and with piped stdin. Agents keep using
  `--title/--notes`, which do not change.
- `--run` continues after creation in this order:
  1. **Run.** Runs `lf task run <ID> [GRAPH]` itself as a child process: it places
     the worktree, takes up the graph (`--graph`, otherwise your `lf set graph` →
     Wave → repository `graph:` → `code`), and returns once the Graph registers.
     Placement, transport release and `-b` waiting stay on that one path.
  2. **Admit the conversation.** `primary::ensure_task` gets an optional
     `request: Option<String>`. It is used only when a new primary is admitted, and
     it fills `LfSession.request`, which `conversation_launch_args` already sends as
     the first turn after the `task/session` skill (in place of `PRIMARY_MESSAGE`).
     Admission happens after the run, so the conversation's cwd is the Task
     worktree and its context already contains `TaskGraph::guidance` ("Graph code:
     running from start…").
  3. **Open it.** When stdin and stdout are a terminal and `--json` is not given,
     serve the conversation in this terminal (`lf resume --task <ID>` behavior).
     Otherwise, the conversation is admitted but not started, and the command prints
     `lf resume --task <ID>` (text) or the receipt (`--json`).
- `-b` together with `--run` behaves like `lf -b task run`: every step runs headless
  and the command waits for `end`. No conversation is admitted.
- `--graph` requires `--run`. Graph capture still happens at take-up; create stores
  no graph.
- `--json` prints the receipt `{ task: <existing create JSON>, graph, session_id }`;
  `session_id` is null for a `-b` run. The run's own lines go to stderr. This is a
  new wire DTO with Rust and Swift fixture tests under `tests/fixtures/dto/`.

### Default workflow read

The sheet has to name the workflow before any Task exists, so the resolver works
from a scope instead of a Task: `scope_graph_default(repo, wave: Option<&str>)`
(main's config layers through `graph_setting`). Both callers use it:
- `task_graph_default(store, task)` → `select_task_run` (unchanged behavior)
- `lf wave graph show [WAVE] [--no-wave] --json`, which gains
  `default: { name, source }` (`SetupSource`: `personal · global · wave · repo · default`; the sheet labels `personal` and `global` as yours), and whose
  `wave_id`/`wave` are null for `--no-wave` (the repository scope).

Desktop shows this read. It never rebuilds the Wave → repository → `code` fallback
chain itself.

### Desktop

- `DesktopAction("new-task", "New Task", keys: ⌘N)` goes in the action registry, so
  the File menu (`ConsoleCommands`), the Keyboard Shortcuts window and
  `lf desktop new-task` all reach it. ⌘N is currently unbound; ⌘1–9 remain `goto`.
- A **New Task** row is always visible at the top of `WorkTaskList` for the current
  sidebar scope. It is not hover-revealed like New Wave, because this is the main
  entry point.
- `NewTaskSheet`:
  - multi-line request field (focused)
  - Wave picker: the repository plus `model.visibleRoadmaps`, defaulting to the
    sidebar scope
  - Workflow picker built from `model.graphCatalog` (existing `WorkflowCatalogEntry`
    and `WorkflowDefinition` supply the interactive-step line), defaulting to the
    `wave graph show` default
  - Create button (⌘↩), disabled while the request field is empty or a create is in
    flight
  - Errors appear inline in the sheet, and the text you typed stays.
- `RegistryQuery.createTask(request:wave:graph:cwd:) -> TaskCreateReceipt` runs
  `task create <request> [--wave W | --no-wave] [--graph G] --run --json`. On
  success, `WorkModel` refreshes and calls the existing
  `openTaskConversation(identifier)`. That finds the Task's primary through
  `prepareTaskSession`, which is the admitted session, and launches it with the
  request as its first turn.

### README

In section 1, replace the `create` / `-b task run` example lines with the one-line
`lf task create "…" --run`, and keep `lf resume --task <ID>`. Keep a single line for
the `-b … --graph auto` variant. Leave the surrounding prose as Jack wrote it.

## Risk findings

- **Can the conversation's first turn hold the request?** Yes, without new plumbing.
  `LfSession.request` already replaces `PRIMARY_MESSAGE` in
  `conversation_launch_args` (`ops/human_session.rs:662`). Continuations already
  use it the same way (`human_session.rs:1035`).
- **Will the conversation see the running Graph?** Yes, if it is admitted after
  `task run` registers. `bind_selected_work` appends `TaskGraph::guidance` at
  launch (`ops/run.rs:239`), and for a running position it says "A person takes
  part again when that run stops."
- **Does `auto` reach a merged PR?** `auto.yaml` = `pursue` + `deliver`, with no
  interactive step. A plain `task run` therefore reaches `end`, which approves
  Auto-Ship (#1733). The conversation still opens and can watch.
- **Could the conversation adopt the wrong session?** `ensure_task` prefers an
  existing chosen primary or an unfinished interactive conversation. A just-created
  Task has neither, and Graph step sessions are headless: `task_conversations`
  reads only `interactive=1` Sessions, so a step Session is never adopted.
- **Will the operator do the Graph's work twice?** The `task-session` operator could
  start editing in the same worktree while `implement` runs. The guidance line
  already says the run is in progress. The demo checks that the first reply reports
  the run and does not edit. If it does edit, wrap the request in a short
  `<lf:task-started>` block that names the Graph and run and leaves building to it.
- **Run fails after create.** The Task exists and the error names
  `lf task run <ID>`. Desktop selects the Task anyway, so its existing
  Start/Run again control is how you recover. No conversation is admitted until a
  run registers.
- **Long titles.** The first line of a long request becomes a long title in the
  sidebar and Linear. This is accepted for now (`lf task edit`), and generated
  titles are excluded.

## Alternatives considered

- **Keep `create` + `run` and let Desktop run both, plus `connect --message`.**
  Rejected. The terminal would need three commands with the request typed twice,
  and Desktop would carry a sequence the terminal doesn't have. That breaks "Desktop
  adds no private path".
- **New `lf task new` / top-level `lf new`.** This would be the nicest demo
  spelling, but it adds a fourth way to make Tasks next to `create`,
  `capture-tasks` and Linear. `create --run` extends what already exists. Renaming
  later is cheap.
- **Use the Task's notes as the conversation's first turn for any first
  primary.** No flag would be needed, but agent-written briefs (like this Task's)
  would appear as your own turn, which misattributes authorship. The request is
  used only when a person typed it into this command.
- **Desktop computes the default workflow from `WaveRoadmap.graph` plus repo
  config.** Rejected because it duplicates the Rust resolver chain.

## Scope

**User-visible outcome:** in Desktop (⌘N / New Task row) and in the terminal
(`lf task create "…" --run`), one action files the Task in the chosen Wave or the
repository, starts its chosen or default workflow, and opens its conversation with
the request as the first turn. The conversation and the Task header show what is
running and where it will stop for you.

**Source of truth:** the Task row (title/notes), the `task_graphs` row (graph +
position, written only by `task run`), and the Task's primary `LfSession` (its
`request` is the first turn). The sheet's Workflow line, the Task header and the
conversation context are all derived from these.

**Affected surfaces:**
- `lf task create` (Clap + handler + routed planning path at `bin/lf.rs:1962`)
- `lf wave graph show`
- `primary::ensure_task`
- `task_graph_default`
- `docs/lf-reference.md` (regenerate)
- README section 1
- the loopflow skill's Task start line, if it shows the three-command start
- Swift `RegistryQuery`, `WorkModel`, `DesktopActions`, `WorkNavigator`/`WorkTaskList`,
  and the new `NewTaskSheet`
- DTO fixtures for the receipt and `WaveGraph.default`

**Absent/error states:**
- Empty request: Clap/sheet refusal.
- Unknown `--graph`: refused before the Task is created (validate with
  `CapturedGraph::load` first), so no orphan Task is left.
- Missing Git `user.email`: the existing claim error, shown in the sheet.
- Graph catalog unavailable: the Workflow picker shows the default name only.
- Run failure after create: the Task stays and you get the retry instruction.

**Operational boundary:** the sheet's Create returns once the Graph registers
(`task run` semantics). The conversation pane opens with no extra click.

**Exclusions:**
- generated titles
- creating on another Machine (`--machine`/`--rent` with `--run`; use `lf task run`)
- iOS
- capturing Tasks from an existing conversation (LOO-368)
- Wave view placement (LOO-594)
- proactive notification when a Graph stops at an interactive step

## Current → target architecture

| | Current | Target |
|---|---|---|
| Start a Task | `create`, `run`, `resume --task` (three commands, request typed twice) | `create "…" --run`. The three commands still work individually. |
| Task conversation first turn | `PRIMARY_MESSAGE` | The request when admitted by `create --run`, otherwise `PRIMARY_MESSAGE` |
| Default graph resolver | `task_graph_default(store, task)` | `(wave_id, repo)` resolver shared by `task run` and `wave graph show` |
| Desktop entry | none | `new-task` action (⌘N, menu, `lf desktop`), New Task row, `NewTaskSheet` |

**Delete — do not maintain:** nothing is being replaced. There is no earlier New Task
UI in Swift history (`git log -S"New Task" -- swift` is empty). The README's
two-line create/run example is replaced, not kept beside the new line.

**Forbidden outcomes:**
- Desktop running its own `create` → `run` → `connect` sequence
- Desktop computing the default graph
- a second "new Task" command
- storing the chosen graph on the Task before take-up
- the request copied into a second persisted field (it lives in the Task's notes and
  the Session's `request`)

## Implementation status (2026-10-28)

Both slices are built, synced onto main after #1751 (`lf set`; `lf wave graph set` is gone).

- `create --run` runs the real `lf task run` as a child process, then admits the primary
  with `primary::ensure_task(request)`, then serves `lf resume --task` when stdin/stdout
  are a terminal. Graph step Sessions are never adopted (`task_conversations` reads only
  `interactive=1`).
- `scope_graph_default` is the one resolver for `select_task_run` and `wave graph show`;
  `WaveGraph` reports only `default` (the redundant `graph` field is gone). `check_graph`
  and `select_task_run` share `ops::task::load_graph` for the missing-graph error.
- Concept review (2026-10-28) found Desktop could not tell "nothing filed" from "filed,
  run failed", so Create again filed a duplicate. Fixed: `--run --json` prints the
  receipt (`graph`/`session_id` null) before exiting non-zero; `queryLf` carries a failed
  command's stdout, `RegistryQuery.createTask` throws `TaskCreateRunFailed` with the
  receipt, and `WorkModel.createTask` closes the sheet, selects the Task and puts the
  error on its Start control. The sheet keeps the request only when nothing was filed.
- The sheet is a `WorkNavigation.Palette.newTask` case, so the action monitor stands
  down while it is open. Create is guarded only by its disabled state.
- Second concept review (2026-10-28, a63a9f365 tree): nothing to change, no open
  product choice. Each concept maps to one owner: request → Task notes + primary
  `LfSession.request`; workflow → `task_graphs` at take-up; default →
  `scope_graph_default`. The sheet's terminal line omits `--no-wave` because
  `create` files without a Wave by default.

Check (a63a9f365 tree): `task_create_run`, `wave_resolution_matrix`, integration
`graph_catalog_and_wave_selection` + `dto_fixtures`, clippy, Swift
`NewTaskTests|WorkDestinationTests` (59): pass.

Remaining: demo with a real provider (`code` stops at its interactive steps; `auto`
reaches a merged PR), including whether the first reply leaves building to the
running Graph. Owner: demo/review.

## Slices (one PR)

1. **Rust.**
   - resolver change
   - `create --run/--graph/REQUEST`
   - `ensure_task(request)`
   - `wave graph show` default
   - receipt DTO + fixtures
   - CLI reference regen
   - README/skill lines

   Focused test: an integration test in `tests/integration.rs` (fake provider,
   `TestRepo`):
   - `lf task create "fix the flaky retry test" --run --json < /dev/null` returns a
     receipt.
   - `lf task status <ID> --json` shows graph `code` with a `run` position from
     `start`.
   - The primary Session's request equals the typed text.
   - `--graph auto` takes up `auto`.
   - `--graph nope` creates no Task.
2. **Desktop.** Action, row, sheet, query and model wiring. Tests:
   - a `RegistryQuery` argument test
   - a ViewInspector test on `NewTaskSheet` (defaults to sidebar scope and the
     reported default, Create disabled on empty)
   - a `WorkModel` test that a successful create selects the Task and requests its
     conversation (fake query)

## Done when

- Desktop, fresh repository: ⌘N → type a request → ⌘↩. The Task exists, it is
  selected, its conversation is open with the request as the first turn, and its
  Graph is running. Nothing else is clicked.
- Choosing `auto` in the sheet (or `-b … --graph auto`) carries the Task to a
  merged PR without stopping.
- The README's terminal line does the same in a terminal.

**Gate checks (headless):**
- `cargo test -p loopflow --lib task_create_run < /dev/null`: passes
- `UPDATE_CLI_REFERENCE=1 cargo test -p loopflow --test integration reference_matches_the_compiled_cli < /dev/null`:
  no diff after commit
- the Swift `LoopflowTests` NewTask suites through `scripts/test.py`: passes

The demo (real provider, `code` and `auto`) is for review.
