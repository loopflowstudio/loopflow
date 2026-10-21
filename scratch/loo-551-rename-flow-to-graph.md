# LOO-551: Rename Flow to Graph

Status: draft plan (kickoff, October 21, 2026). Product decisions accepted by
Jack Heart on October 21, 2026 live verbatim in `scratch/loo-551-decisions.md`
and bind this plan.

## Direction after design review (Jack Heart, October 21, 2026)

Jack: "isn't it just renaming lf flow to lf graph" — moving `.lf/flows` to
`.lf/graphs` is the model for the whole change. Treat it as a mechanical rename
everywhere, without special cases:

- No skew errors or "`flow:` is now `graph:`" messages; old spellings simply stop existing.
- Cron renames too: `--flow` → `--graph`, plist keys and `target_kind` values;
  `lf wave cron sync` reinstalls. No exception, no follow-up Task.
- The migration is plain `ALTER TABLE … RENAME` plus the stored edge rewrite; keep it minimal.
- Capture manifests: rename the field; keep `#[serde(rename = "flow")]` only so
  existing on-disk history still replays (shipped records stay immutable).
- `FlowGraph` → `CompiledGraph` stands.
- The manabot companion PR stays (external repo).

## Problem

"Flow" sits next to Loopflow, Workflow and flow state, and undersells the thing:
a composed graph of Skills and commands with loops and XOR routing. The homepage
(LOO-550) already says `lf … graph pursue` and links "Skills and Graphs" to
`/docs/authoring#flows`; `lf`, Desktop and docs still say Flow. Anyone learning
the noun from the website, docs or `lf help` meets two names for one thing.

## Demo

```bash
lf graph list                      # builtin + .lf/graphs/ definitions
lf graph show pursue               # `- graph: refresh` composition, drawn
lf graph customize pursue          # prints .lf/graphs/pursue.yaml
lf graph pursue                    # runs it; `lf pursue` still works
lf flow list                       # error: no Skill or Graph named flow
lf task workflow show LOO-551      # edges read `graph: pursue`
```

Desktop: the Task work pane says "Graph processes", the palette offers
"Graph definition", the catalog inspector reads "builtin Graph". The website's
"Skills and Graphs ↗" lands on the `## Graphs` heading of the authoring doc.

## Naming map (accepted direction → concrete names)

| Before | After |
|---|---|
| `lf flow list/show/customize/<name>` | `lf graph …` (`lf <name>` shorthand unchanged) |
| `.lf/flows/*.yaml`, `builtins/*/flow/` | `.lf/graphs/`, `builtins/*/graph/` |
| composition step `- flow: refresh` | `- graph: refresh` |
| Workflow edge `{ from, to, flow: pursue }` | `{ from, to, graph: pursue }` |
| `FlowDefinition`, `FlowComposition(Item)`, `FlowCatalogEntry`, `FlowRunner`, `FlowCommand`, `FlowProcess*`, `FlowStepProcess`, `FlowReturn`, `FlowProgress`, `FlowOutcome`, `FlowVerdict`, `FlowDecision`, `FlowTransition`, `FlowInput`, `FlowOutput`, `FlowHeld`, `FlowNodeInput`, `FlowContextPreview`, `ResolvedFlow*`, `RunningFlow`, `SessionFlowOccurrence`, `SessionFlowMembership` (DTO) | `Graph…` equivalents |
| `FlowGraph` (display projection of a compiled definition), `FlowGraphPath` | `CompiledGraph`, `CompiledGraphPath` |
| `FlowNode`, `FlowNodeKind` | `GraphNode`, `GraphNodeKind` |
| field `graph: FlowGraph` on catalog entries, process detail, context preview | `compiled: CompiledGraph` |
| Swift `FlowDiagram`, `FlowGraphView.swift`, `Flow.swift`, `FlowPalette`, `FlowLoopGeometry`, `FlowNodeState`, `FlowCatalogInspector`, `FlowProcessView`, `FlowProcessLog`, `FlowCompositionView/Projection` | `GraphDiagram`, `GraphDiagramView.swift`, `Graph.swift`, `Graph…` |
| Rust module `src/flow/` (`graph.rs` inside it) | `src/graph/` (`compiled.rs`), `ops/flow_process.rs` → `ops/graph_process.rs`, `store/sqlite/flow_inventory.rs` → `graph_inventory.rs`, `lf/commands/flow.rs` → `graph.rs` |
| wire fields `latest_flow_process`, `flow`, `flows`, `flow_membership`, `flow_lf_process_id`, `flow_processes`; values `"flow"` (definition/palette/context kind), `"flow_log"` | `latest_graph_process`, `graph`, `graphs`, `graph_membership`, `graph_lf_process_id`, `graph_processes`; `"graph"`, `"graph_log"` |
| `StoreRevisions.flows`, revision domain `'flows'` | `graphs`, `'graphs'` |
| `LF_FLOW_ID` | `LF_GRAPH_ID` |
| DTO fixtures `tests/fixtures/dto/flow_*.json` | `graph_*.json` |
| generated skill front matter `loopflow-kind: flow`, `loopflow-flow:` | `loopflow-kind: graph`, `loopflow-graph:` (files are found by `loopflow: true`, so sync replaces them) |
| "Flow process" (one lf process running a Graph) | "Graph process" |

Workflow, `WorkflowDefinition`, `task_workflows.graph` and `.lf/workflows/`
keep their names (decision). Desktop's Workflow drawing becomes
`WorkflowDiagram` (`WorkflowDiagram.swift`, `WorkflowDiagramLayout`), parallel
to `GraphDiagram`: with Graph naming the definition, `WorkflowGraph` reads as
"the Graph a Workflow edge runs" (concept review).

Unchanged by decision: musical "flow" in product copy ("the creator's flow"),
the Loopflow name, and the "loopflow" pun ("A **loopflow** is a Graph with one
or more backward edges" — the authoring heading stays "Loopflows").

## Risk findings

1. **SQLite rename is mechanical and safe on 3.51.** Probe on a schema-only copy
   of the live store: `ALTER TABLE … RENAME` and `RENAME COLUMN` rewrite the
   dependent index, the `validate_flow_process_step` body and the
   `store_revision_*` triggers; `json_each`/`json_set`/`json_remove` rewrites
   `task_workflows.graph` edges `flow` → `graph`, including `"flow": null`.
   Object *names* and RAISE messages still say flow, so the draft drops and
   recreates them. `task_workflow_graph_is_fixed` blocks `UPDATE OF graph`; the
   draft drops it around the rewrite and recreates it. `flow_processes` rows need
   no data change (compiled JSON carries no `flow` key), so its append-only
   triggers are only renamed.
2. **`store_revisions.domain` has a CHECK listing `'flows'`.** Rebuild the table
   with `'graphs'`, carrying the counter value, and recreate every trigger that
   bumps that domain.
3. **`wave_workflows.content` is authored YAML text** (4 rows locally, edges in
   flow style `{ from: start, to: end, flow: x }`). The draft rewrites ` flow:`
   and `{flow:` by text replace, which leaves `workflow:` alone and covers both
   inline and block edges; it does not re-parse the rows.
4. **Session capture manifests are shipped history on disk.** `manifest.json`
   carries `"flow": {"kind": …}`; 4,851 observed `session_events` rows copy it
   and the `session_input_membership` index reads `$.evidence.flow.kind`.
   New captures still write that key. Following the `exec` precedent
   (`#[serde(rename = "exec")] // Existing capture manifests remain replayable`),
   the Rust field becomes `graph` with `#[serde(rename = "flow")]`; one spelling
   on disk, no reader for two. The index is untouched.
5. **Cron's installed contract is machine-local and spelled flow.** Launchd
   plists store `LoopflowFlow`, `LoopflowTargetKind=flow`, and a
   `LoopflowCommand` of `["--wave", …, "--batch", "flow", "<name>"]`; receipts,
   obligations and dispositions under `~/.lf/cron/` store `flow` and
   `target_kind: "flow"`. Its `--flow` flags name "Graph or Skill" targets, not
   Graphs. After this change an installed command that runs `lf flow` fails;
   `lf wave cron sync --wave <wave>` reinstalls from `.lf/config.yaml`. See
   Decisions/assumption A2.
6. **One external repository carries Flows:** `loopflowstudio/manabot`
   (`~/src/manabot`, mirrored as `~/src/fantasia`) has 7 files in `.lf/flows/`,
   4 using `flow:` steps. Cube, Etude, Kata and Hootro have none. Its branch
   worktrees migrate when they rebase.
7. **No reader classifies Flow processes by argv.** Inventory, monitor and Task
   work read `flow_processes`, so historical `["lf","flow",…]` argv rows in
   `processes` stay history and lose nothing.
8. **`~/.lf/flows/<uuid>/driver.lock`** directories (last written October 4) have
   no writer in the source; they are inert leftovers and out of scope.
9. **`## Flows` sections in Linear Project descriptions** are authored text the
   chapter code preserves without parsing; leave them.

## Approach

One indivisible PR. A partial rename leaves two nouns, which is the bug.

### Deletion cut (first)

- Delete `lf flow` (`FlowCommand`, `Target::Flow`, navigation owner `"flow"`),
  `.lf/flows/` loading, builtin `*/flow/` directories, the `flow:` step key and
  edge key parsers. No alias, no dual reader.
- No skew detection: old spellings stop existing (review direction).

### Build on what remains

1. **Rust** — `git mv` modules/files per the map; rename types, fields,
   functions, tests, help text and messages. `build.rs` reads `graph`
   directories. `lf list` and `lf help` group "Graphs".
2. **Draft migration** `rename_flow_to_graph`
   (`uv run python scripts/new_migration.py rename_flow_to_graph`): tables,
   columns, index, triggers, `store_revisions` domain, `task_workflows` edge JSON,
   `wave_workflows` YAML. Upgrade test
   (`graph_inventory::released_flow_records_read_as_graphs_after_the_rename`)
   seeds the released frontier from `tests/fixtures/migrations/graph_rename_released.sql`
   (a Flow process with one step, a Task mid-edge with `flow: pursue`, a Wave
   workflow), applies the draft, and reads them back through `SqliteStore`:
   Graph process, `graphs` revision, `graph` edges, rewritten Wave YAML.
   The draft is a rename, so the cut raises the reader floor (expected; goes to
   `release/unreleased/BREAKING.md` at cut time).
3. **Wire/DTO** — rename fields and kinds; rename fixtures; update
   `tests/dto_fixtures.rs`, Swift `DTOFixtureTests`. No defaults.
4. **Swift** — models, views, files, user strings, CLI argv
   (`RegistryQuery`: `["graph", "list", "--processes", …]`).
5. **YAML** — this repo's `.lf/flows/` → `.lf/graphs/` (9 files), builtins, all
   `flow:` steps/edges, `.lf/config.yaml` cron commands (`graph telemetry-daily`,
   `graph release-run`, `graph wave/operate`).
6. **Prose** — AGENTS.md, README.md, TESTING.md, PROMPTS.md, `docs/**`
   (authoring `## Graphs` with `.lf/graphs/` examples, config, lf.md,
   architecture*, glossary, waves, troubleshooting), builtin skills and
   `builtins/LOOPFLOW.md`, `swift/README.md`/`DESIGN.md`, website
   `content.yaml`/`home.html` link → `/docs/authoring#graphs`, scripts
   (`check_architecture.py`, benchmarks, evals, bootstrap-cron-host.sh), e2e
   tests, goldens (regenerate). `docs/lf-reference.md` regenerated per AGENTS.md.
7. **manabot companion PR** — `git mv .lf/flows .lf/graphs` and `flow:` →
   `graph:` in `loopflowstudio/manabot`, opened when this PR is ready to land;
   it merges after an lf release containing the rename.

## Absent and error states

- Repo with only `.lf/flows/`: its definitions are simply not found; builtins still load.
- `lf flow …`: falls to the `lf <name>` shorthand and fails as an unknown
  Skill or Graph; no alias catches it.
- Installed cron whose saved command runs `lf flow`: the run fails with the
  ordinary unknown-command error in its receipt; `lf doctor` cron section and
  the cron receipt name `lf wave cron sync --wave <wave>`.
- `wave_workflows` rows are rewritten by text replace (` flow:` / `{flow:`);
  the draft does not re-parse them.

## Exclusions

- Release notes, `release/v*/`, applied migrations (decision).
- Capture manifest on-disk key `flow` and its index (finding 4); the Rust field is `graph`.
- `.lf/wave/*/MEMORY.md` and `GOAL.md` (≈100 Flow mentions, mostly
  infrastructure and product memory): Wave context is maintained by its Wave
  procedure, not this Task. Proposed edit: a single line per memory noting
  "Flow was renamed Graph in LOO-551", applied by the next Wave review.
- Workflow naming (decision).

## Forbidden outcomes

`lf flow` alias; reading `.lf/flows/` or `flow:` alongside the new names; serde
`alias = "flow"` on any DTO; Swift decoding both keys; keeping old fixture files;
a second migration draft; "Flow" left in `lf help --all` or Desktop strings.

## Done when (gate, headless)

- `lf graph show pursue --json | jq '.compiled.steps | length'` > 0 and
  `lf graph customize pursue` prints `.lf/graphs/pursue.yaml`; `lf flow list`
  exits non-zero naming no Skill or Graph `flow`.
- `LF_HOME=$(mktemp -d) target/debug/lf help --all --json | grep -ci '\bflows\?\b'`
  counts only lines whose text is musical flow (expected 0).
- `cargo test` (integration + `dto_fixtures` + migration upgrade test),
  `cargo clippy --all-targets -- -D warnings`, `uv run pytest python/tests/`
  (architecture, skill alignment), Swift `DTOFixtureTests` and
  `DesktopHeadlessTests`, website `tests/e2e/test_homepage.py` asserting the
  `#graphs` anchor exists in the rendered authoring doc.
- Residual-noun check: `git grep -nE '\b[Ff]lows?\b' -- ':!RELEASE_NOTES.md'
  ':!release' ':!rust/loopflow/src/store/migrations/0*' ':!.lf/wave'` returns
  only musical/brand uses, the pinned manifest/cron spellings, and their
  comments.
- On a copy of the real store (draft build path per MIGRATIONS.md),
  `lf task workflow show LOO-551` shows `graph:` edges and Desktop's Task work
  pane lists Graph processes.

## Remaining work

Exists (October 21, 2026): file moves (incl. `WorkflowDiagram.swift`); rename across Rust, Swift, YAML, docs and
website; draft `rename_flow_to_graph` and its upgrade test; manifest
`serde(rename = "flow")` pins; regenerated `docs/lf-reference.md` and prompt
goldens. Kept as history: Linear Project description `flow:` declarations
(`linear/project_document.rs`), `legacy_flow` session events, auth-flow names.

Left:
- Gate: full `cargo test` (incl. `cli_reference` after the v0.14.14 merge),
  Python, Swift `DTOFixtureTests`/`DesktopHeadlessTests`, website e2e `#graphs`
  anchor, residual-noun check, copy-of-real-store `lf task workflow show`.
- manabot companion PR at landing.

## Land-to-release window (concept review, October 21, 2026)

Installed lf (0.14.12) reads this checkout as follows, verified with
`command lf` here; main behaves the same from landing until the release is installed:

- Repo Graphs (`.lf/graphs/`) are invisible: `lf flow show release-run` →
  "flow not found". Builtin Graphs still run from the binary. Installed cron
  plists keep running `lf flow telemetry-daily`/`release-run` and fail until
  the release is installed and `lf wave cron sync` runs (accepted: cron renames too).
- `.lf/workflows/autonomous.yaml`'s edge parses as `"flow": null`, a
  graphless start → end edge. Taking it up with installed lf
  (`lf wave workflow set autonomous`) would capture an edge that runs nothing
  and requests completion. Stored Wave/Task Workflows are unaffected (DB copies
  still say `flow:` until the migration runs).
- Cut the release right after landing with `lf repo release run patch`
  directly (what `release-run.yaml` runs), not via the repo Graph, and avoid
  selecting the `autonomous` Workflow until it is installed. Jack ruled out
  compatibility exceptions, so this is sequencing, not a shim.

## Decisions

- 2026-10-21, Jack Heart: see `scratch/loo-551-decisions.md`.
- 2026-10-21, kickoff (reversible): `FlowGraph` → `CompiledGraph`, field
  `compiled`, column `graph_processes.compiled`, because "graph" now names the
  definition. Assumption A1.
- 2026-10-21, kickoff (reversible): capture manifests keep `flow` on disk.
- 2026-10-21, Jack Heart (design review): cron renames too (`--graph`,
  `LoopflowGraph`, `target_kind: graph`); `lf wave cron sync` reinstalls.
  Supersedes A2.

## Checks

Kickoff: SQLite 3.51 rename/JSON probe on schema copy of live store — passed.
Compress: clippy -D warnings, `cargo test --lib graph_inventory`, Swift `build-for-testing` (fixed merged-in `WorldReadTests` Flow queries) — passed.
Implement: clippy -D warnings, upgrade test, `dto_fixtures`+`graph_discovery`, `lf graph show pursue` (8 steps), `lf flow list` exit 2 — passed. Gate owns the rest.
