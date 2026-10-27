# LOO-622: one Graph, two ways to run it

Jack Heart's direction (2026-10-26). Accepted decisions:

- There is no Workflow distinct from a Graph. A Task runs a Graph.
- A step is `interactive` or not. One word, everywhere; `human` naming goes.
- `lf graph <name>`: at an interactive step, launch an interactive coding agent
  in place, then continue.
- `lf task run`: at an interactive step, stop; the Task waits on a person in its
  conversation. That stop is the only thing "Workflow" meant.
- `-i` / `-b` override every step's interactivity for the launch, for `lf skill`,
  `lf graph` and `lf task run` alike. `-b` has one meaning: run headless, print
  output, return when the work ends. `lf -b task run` therefore runs straight
  past interactive steps: Auto-Ship mode.
- `-a` overrides per-step `agent`. Machine stays per run (`--machine`); no
  per-step machines.

## Demo

`lf graph pursue` in a terminal: design step opens an interactive agent, the
rest run headless. `lf task run LOO-x pursue` stops at design and the Task
shows it waiting in its conversation. `lf -b task run LOO-x pursue` runs
through to the PR without stopping.

## What the code does today (inventory 2026-10-26)

- `lf graph <name>` does not launch an interactive agent at an interactive step;
  it stops there with exit 0 (`graph/runner.rs:133`, `GraphOutcome::Staged`).
  `-i`/`-b` only choose how the headless steps' agents run.
  It used to work (git history): until #1175 (2026-08-15) a skill with
  `interactive: true` launched as an interactive Session when a Flow reached it.
  #1175 turned `human: true` nodes into durable Asks (direct TTY stayed
  human-present); #1439 (2026-10-07) made `lf graph` refuse human steps and
  Tasks park at review; #1652 (2026-10-27) made it stop at the stage. Restore
  the pre-#1175 behavior; don't revive Asks.
- No `human` node exists. `human:` is the retired spelling of
  `interactive: <skill>` (`graph/mod.rs:756,848`); fixtures still say `human`.
- "Workflow" is a stored projection of a Graph: `task_workflows` (graph JSON,
  node, edge, lf_process_id) + `task_workflow_moves`; stages = interactive
  steps, routes = slices between them, each run as child `lf graph route`.
  Task state (`active`/`moving`) is computed from that row (`durable.rs:460`).

## Delete

- `workflow.rs` projection (nodes/edges/routes) and its duplicate,
  `InteractionGraph` stages/transitions in `graph/compiled.rs`.
- `ops/workflow.rs`, `task_workflows`, `task_workflow_moves`, `waves.workflow`
  (one draft migration; fold the `ship_it` draft's rewrites into it).
- `lf task workflow move|show|restart`, `lf wave workflow …`, `lf graph route`,
  `lf task run <issue> end`, `workflow_catalog` (keep `graph_catalog`).
- `placement_in` heuristic; World's separate `task_run`.
- Desktop: `TaskWorkflowHeader`/`TaskWorkflowBar` route UI, `WorkflowCatalog*`,
  `task workflow move`; DTO fixtures that carry `workflow`.
- Docs/skills Workflow sections (authoring.md, lf.md, glossary, waves.md,
  architecture, task_operate/wave_operate/work skills, AGENTS.md).
- Retired-frontmatter messages point to `interactive: <skill>` steps; rename
  `human` fixtures.

## Built (2026-10-26/27)

- Runner: `interactive:` steps run in place under `lf graph` (launch like
  `lf skill`: terminal → interactive, else headless); only a Task run stops
  before them. `-b` runs every step headless with a stand-in note; `-i` every
  step interactive. (commit 459bf9daa6)
- `task_graph.rs` replaces `workflow.rs` + `ops/workflow.rs`; `task_graphs`
  draft replaces `task_workflows`/`task_workflow_moves` (and rebuilds
  `task_state_deliveries` without `move_seq`); `waves.workflow` → `graph`.
- `lf task run [GRAPH] [--from STEP]`; default graph Wave → repo `graph:`
  (alias `workflow:` until LOO-637) → `code`. The child (`lf graph run`,
  hidden) writes the arrival. `lf ship` = repeated `-b task run`.
- Deleted: `lf task workflow …`, `lf wave workflow list|source` (`show|set`
  → `lf wave graph`), `lf graph route`, `lf task run X end`, move
  explanations, `run_task_skill`, ship's per-stage stand-in routing and
  blocked backoff.

## Remaining

- Demo: the three `pursue`/`feature` runs above, in a temp LF_HOME.
- Lib suite never ran clean in one pass under load (90 failures, all green on
  rerun; git-email/timeout flakes predate this work) — CI is the gate.

## Decided: the Task's Graph keeps a table (Jack Heart, 2026-10-26)

People run arbitrary Skills and Graphs inside a Task worktree without moving
the Task's main Graph, so its position can't be derived from the latest Graph
process. One row per Task: the captured graph and the step it waits at (plus
the process carrying it, if running). Only `lf task run` moves it; `lf graph`,
`lf skill` and `lf --task … graph` in the checkout never do. Replace
`task_workflows` with that shape; drop node/edge/route vocabulary and decide
whether the moves history earns its place.

## Decided: default Graph is `code` (Jack Heart, 2026-10-26)

A Task with no Graph takes up the builtin `code` on its first `lf task run`.
Wave and repository overrides stay as `graph:` (renamed from `workflow:`;
`.lf/config.yaml` keeps reading `workflow:` until release, with a removal Task).

## Decided: `--from` replaces moves (Jack Heart, 2026-10-26)

`lf task run <issue> --from <step>` starts the Task's Graph at that step and
runs from there. It replaces `lf task workflow move|restart` and the Desktop
move menu.
