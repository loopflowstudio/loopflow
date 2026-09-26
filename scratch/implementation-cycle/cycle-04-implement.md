# Cycle 4 — implement: Task Flow preview, pinned diagram, controls

2026-09-25, after the #1283 rebase (main `5bcc40fdf`). Reconciled the
interrupted draft (`engine/flow_graph.rs`, `ops/task_flow.rs`, the
`task_execution_and_flow` read and `recommended_task_flow`) and finished the
slice. No commit, publication, install, PM write, or live Session/Task action.

## Observable claims

- **Shared read.** Every roadmap/status Task carries required `flow:
  TaskFlowSnapshot { recommended, record, controls }`. `record` is `none`,
  `pinned` (captured graph, current occurrence key, pass-scoped `completed`,
  per-edge `returns[{decider,target,traversals}]`, execution state/reason,
  `restart_required`), or `finished { flow }` — a finished Flow draws no topology
  because its definition is not retained; today's recommendation is never
  shown as its history.
- **Occurrence identity.** Nodes carry structural keys (`3`, `6/fix/0`) that match
  the saved `ExecutionCursor`; repeated `loop-decide` occurrences are distinct.
  Return counts come from the existing `FlowProgress.repeats`, including settled
  XOR child counts — no new counter or store.
- **Catalogue.** `lf flow list [--json]` expands every available Flow through the
  shared loader; an unexpandable one carries its reason instead of a graph.
- **Restart with a selection.** `lf task restart ISSUE [advice] --flow FLOW`
  extends the one existing restart operation. The replacement is loaded before
  PM refresh, checkpoint, steer, or worker stop.
- **Legality in Rust.** Start/Resume/Restart/Pause each carry `unavailable:
  Option<String>`. Pause is always unavailable with the real reason: `lf task
  interrupt` ends the turn and the worker restarts that step in place
  (`interrupted_step_restarts_in_place_with_fresh_direction`), so nothing holds
  advancement at a saved boundary. No invented pause state.
- **Native Task surface.** `TaskFlowView` sits under the title/condition line,
  before Sessions and Description. Lowercase monospace skill names in one
  continuous row; each authored backward edge gets a pale-blue span and a return
  arrow beneath; centred "Loop" or "Loop · Iteration N" when the cursor is inside
  that span. Completed-this-pass green, running blue, human pending/waiting
  yellow, blocked red, idle neutral. Clicking a node shows real detail (kind, id,
  return target and times taken, composed parents, XOR paths with the selected
  one marked). The Flow name is the typeahead: before start it previews only;
  once pinned it opens the replacement search, then an explicit confirmation
  naming the checkpoint/push/stop consequences. Hover or keyboard focus on the
  name reveals **Stop & restart…**. Failures stay on that Task.
- **Status line distinguishes** "Not started · No runs yet." (no record, no start
  evidence), "No Flow recorded · earlier Runs exist outside a managed Flow"
  (`runtime.started` without a record), finished-not-retained, and pinned state.

## Ownership and deletions

- Topology/keys/cursor projection: `engine/flow_graph.rs`. Control legality:
  `ops/task_flow.rs`. Both read existing `FlowPosition`/`QueuedInvocation`,
  `TaskExecutionSnapshot` and `TaskEvent::FlowFinished`.
- Swift draws only. Drafts (preview choice, picker, pending replacement,
  in-flight/error) live in `WorkspaceNavigation.flowDrafts`, beside the rename
  draft; controls run through `PodiumModel.performFlowControl` →
  `RegistryQuery.{runTaskFlow,restartTaskFlow,resumeTaskFlow}` and settle only the
  originating repository's Task. The catalogue is one per-repository reading,
  loaded once and forced when the picker opens — never per repaint.
- Deleted from `WorkSurfaceView`: the Task overview's `TaskActionCluster` use,
  its `TaskControl` enum, `perform` paths and `activeControlId` (Start/Resume now
  come from the Flow controls; Worktree/PR remain inline).
- Retained overlap: `TaskActionModel.recommended` (one delivery next-move) still
  drives Roadmap/Now rows; it is a different concept from per-control legality.

## Proof

Rust (ambient `LF_*` unset):
- `cargo test -p loopflow --test flow_tests task_flow_read` — 1 pass
  (`/tmp/loo291-c4-cli.log`). Real CLI, isolated Home, bound chapter: `none` +
  Start available; `flow list --json` previews both returns → implement;
  pinned position at iteration 3 with `decide:2`, `decide_delivery:1`; source
  edited afterward — roadmap still draws the pinned six-node definition, current
  `1`, completed `["0"]`, counts `(2→1:2),(4→1:1)`; `task restart --flow
  missing-flow` fails naming it, FlowPosition byte-equal, HEAD unchanged; a
  restart-required blocker projects `blocked` and refuses Resume.
- `cargo test -p loopflow --lib -- ops::task_flow engine::flow_graph lf::tests
  ops::task::tests::task_worker` — 60 pass (feature topology, pass-scoped
  completion, XOR keys/counts, control matrix, fixture round-trip + missing field).
- `cargo test -p loopflow --test dto_fixtures` — 8 pass.
- `cargo clippy --all-targets -- -D warnings`, `cargo fmt --all --check`,
  `git diff --check` pass.

Swift: `swift test --package-path swift -Xswiftc -gnone --jobs 4 --no-parallel
--filter 'TaskFlowTests|TaskFlowProofTests|DTOFixtureTests|WorkspaceNavigationTests|TaskDirectiveEditorTests|WaveLensTests|NowProjectionTests|RoadmapViewTests|PodiumModelTests|WorkspaceNavigationProofTests/namedSessionDrillDownRetainsTerminal'`
— **85 tests / 11 suites pass** (`/tmp/loo291-c4-swift.log`).
- `flowControlsRetainTerminals` (mounted `SessionsView`, real Ghostty, two
  `/bin/cat` PTYs, fixture transport that records controls): unstarted preview
  with both loop spans; typeahead filter, Cancel keeps `feature`; choosing
  `build` records nothing; Start records exactly `task run W2-156 --flow build`;
  pinned W2-131 shows demo waiting, implement completed, "Loop" vs "Loop ·
  Iteration 2", Resume disabled; restart Cancel records nothing; confirmed
  restart is refused and the error shows while the graph stays pinned; a
  refreshed read moves execution to running review-slice and exposes a disabled
  Pause; the Session draft and companion then reply and the layout is unchanged.
- Native captures: `cycle-04-evidence/task-flow-pinned.png` and
  `task-flow-running.png` (fixture reads, hidden window,
  `LOOPFLOW_FLOW_CAPTURE_DIR`). Inspected: one continuous row, both return
  arrows into implement, nested spans with the outer iteration label.

Counterexamples: the first roadmap read showed `tasks: unavailable` (fixture had
no chapter binding / branch) — test now binds a chapter and creates the branch.
`lf flow list --json` first failed clap parsing; `Flow` gained a local `--json`
flag (ignored when running a Flow). The first native attempt could not see
view-local `@State` through ViewInspector; drafts moved to the existing
navigation owner (same pattern as rename). The existing
`namedSessionDrillDownRetainsTerminal` failed deterministically here: its wait
loop exited before the async rename started (debug trace showed the refusal did
arrive). Fixed the wait to await the refusal itself; removing `TaskFlowView`
did not change that failure, so it is not caused by this slice.

SHA-256 (final):

```text
engine/flow_graph.rs      feec69ea405a6582eb6d142bf3000a017815346490b14a22dea7e243dad92324
ops/task_flow.rs          9d8d02bb483b456318fe387e7cbc163734250fec5183bd42ca299c4a9b96d03b
ops/task_execution.rs     d91cd634501dab079141ac9c8d804839c959e44b33c68d7c00a9113eda9a4ff8
ops/task.rs               692d2fca71acd77d2ec7156e43e5589633ec16e6393ed50b79b57d2dd26f7a80
lf/commands/waves.rs      f2407c32b0930ffbc818fe51294d8e7822e9d0bfb70f449f3dbf5fe7d36e2788
lf/commands/flow.rs       a34e2e7219991f95de4f624ca3a4dee0db9c2d468a1d986b61618b6a453e3d42
bin/lf.rs                 bdfccbee75194154c4a1ca7d8187aecf660c844988917cf3c72b6af311e19624
lf/mod.rs                 0e00d2e5f4b2523629791c5dfab53bce341c83df1deb38f93db0785b6aa43bb3
tests/flow_tests.rs       c5dddbf5a10cb899572383bd841a3faf0996751f408ce8797f0a8bf27f3f0cc5
TaskFlow.swift            15be962b88e4e829ea0c6b5c4a2c7cd784af33eeb0fec37d3b2a5cab3627563d
RegistryQuery.swift       4ac2b888917c77248b44c1ace39c71400b82553997e1aa17341c87198face23e
PodiumModel.swift         460e2176c489dc3d37cb5b870fd56593a71a984387fc8da3db3aaf3245b487b9
WorkspaceProjection.swift 100a1cf0643f8497db6212e072da3226ee043c9fd41d777b8294a37a19c2cf9b
TaskFlowView.swift        924eee8d4ae2a764a9e37e0dbb0dff14aa31db926623ec707e8c5e7d48e9eb25
WorkSurfaceView.swift     4886f061b4148c7b631dc531309c16b006e08287908669a92c0fe02901973cdb
TaskFlowProofTests.swift  41845a63c402dfcf8547e90693f8d1503ea1be77b077e2ca2b57f08ade9d2a5a
task_flow.json            5a5fb74c4e8e484d1d52e3be959d3d499ce5313495e864befb0bd29f20ded26b
flow_catalog.json         d75defb7ef1ff9ac94fcd6700a11c274754c8e5c51785ac8e642501c30d3ce6a
```

Fixtures `roadmap_snapshot.json`, `wave_detail.json`, `task_condition_states.json`
gained `flow` on every Task (additive only). Docs: `docs/lf.md`, `swift/README.md`.

## Limits and remaining complete-design gaps

- Not run: real `lf task run/restart` against a configured PM/provider (restart
  success path needs Linear refresh + checkpoint/push; only the pre-side-effect
  rejection is proven), Xcode fallback build, hosted UI, configured app.
- Hover reveal of Stop & restart is not exercised by ViewInspector; the proof
  reaches the same picker by the clickable pinned Flow name.
- Nested XOR returns are counted and listed in node detail but not drawn as
  arrows; a selected XOR path's current step is shown by marking the XOR node and
  its detail, not expanded inline.
- Blocked help remains the Session/Ask path; the Flow section offers no
  "help"/Advance control.
- `recommended` uses the cached chapter plan; restart re-resolves it with a
  forced PM refresh, so the two can differ after an unsynced plan change.
- Visual: completed nodes read grayish-green under the loop tint; the long
  Feature row scrolls horizontally at 920pt. Both belong in the post-build
  two-loop composition discussion.
- Next: Comments (real count/thread through the planning boundary), then the
  supervised final build (`final-build-plan.md`), configured demo, measurements,
  external trials and authorized edit.
