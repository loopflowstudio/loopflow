# Cycle 2 — implement: named native Sessions

## Result

Selecting a Session now drills the native breadcrumb Wave → Task → **Session
name**. Wave and Task crumbs navigate upward to their details; the Task's issue
ID links to Linear. With several Sessions on a subject, the final crumb is a
menu choosing the exact Session by name. A pencil renames in place through
`lf session rename --json`; the shown name is Rust's readback. Beside the name
the Session shows its exact Flow membership. Monitor stays available in the bar
for Task Sessions. No Description or separate Session-with-context surface.

### Shared exact Flow membership (Rust → Swift)

`SessionRecord.flow_membership` (required) is `step{flow, invocation_id, step,
step_index, iteration, current}`, `independent`, or `unknown{reason}`.

- Flow boundary Sessions: from their `FlowPosition` (always current).
- Every other Session: from its Run manifest's new `flow` field, recorded at the
  owning capture. `RunSpec.flow` is required: the task controller records
  `Step(RunFlowStep::of(position))` for autonomous skill/op steps;
  `prepare_flow_run` records it for human steps (launch cannot reassign it);
  Asks, replays, wave-resident, direct and generic launches record `Independent`.
  `current` compares against the Task's current Flow position.
- Manifests written before this field have none → `unknown` ("predates recorded
  Flow membership"), never `independent`. A failed Flow-position read → `unknown`.

No new registry, index or writer: membership extends the existing Run manifest
at its existing capture transactions. Swift decodes and presents only.

### Naming gaps carried from cycle 1

- **Remote truthfulness:** a remote-Home Flow Session no longer claims a local
  generated title. It projects the step as a label with new
  `title_source: unavailable`; storage rejects writing `unavailable`. The Mac
  hides the pencil and says the name lives on its Home; rename there still fails
  with the routed `lf ssh … session rename` command.
- **Guidance scope:** the rename paragraph moved from universal `LOOPFLOW.md` into
  the human-present surface. Headless Runs no longer receive it; prompt goldens
  are back to HEAD's content.
- **Flow boundary behavior:** new lib test proves a prepared human Flow Run records
  its step, lists as a Flow Session named by its skill, is renamed by
  `$LF_RUN_ID` before any provider history, keeps a human name against a later
  suggestion, rejects a blank name, and after the Flow moves on lists as an
  interactive Session keeping its human name with `step … current: false`.

### Native rename state and ordering

`WorkspaceNavigation.renaming: SessionRenameDraft?` owns the draft for exactly one
Session. `PodiumModel.commitSessionRename()` captures the navigation owner and
target: success bumps the Session read generation (an older poll cannot restore
the old name) and replaces that record in the originating repository's reading;
rejection keeps the typed text and error only if the draft still names that
Session. Selecting another Session abandons an unsubmitted draft; a submitted one
settles against its own Session. `RegistryQuery.renameSession` passes
`--json -- <id> <name>` so a name beginning with `-` is not parsed as a flag.

## Changed paths

Rust: `run_record.rs` (RunFlowStep/RunFlowMembership, manifest + spec field,
`Unavailable` title source rejected on write), `ops/human_session.rs`
(SessionFlowMembership, projection, remote title, fixture test),
`controller/task/mod.rs` (step capture + Flow naming test), `engine/agent.rs`,
`controller/wave/runner.rs`, `lf/commands/{ops/mod,run,replay}.rs` (Independent),
test initializers in `lf/commands/{util,runs,usage,replay}.rs`, `ops/run.rs`,
`run_record/active.rs`, `run_record/active/reader/tests.rs`;
`engine/builtins/LOOPFLOW.md`, `engine/builtins/surfaces/human-present.md`.

Fixtures: `session.json`, `sessions.json` gain `flow_membership`; new
`session_memberships.json` (all five projections); fixture README.

Swift: `Models/SessionRecord.swift`, `Services/RegistryQuery.swift`,
`LoopflowMac/{PodiumModel,WorkspaceProjection}.swift`, new
`Views/WorkspaceBreadcrumbBar.swift` (replaces SessionsView's `taskControls`),
`SessionFixture.swift`, README; tests: new `SessionRenameTests.swift`,
`DTOFixtureTests`, new native case in `WorkspaceNavigationProofTests`, and
`flow_membership` added to Session JSON in eight existing test files.

Existing dirty work preserved; nothing staged or committed.

## Proof

- `cargo test -p loopflow --lib -- human_session run_record::tests controller::task::planning_tests::flow_sessions engine::builtins::tests` — 52 pass.
- `cargo test -p loopflow --test session_cli_tests --test golden_prompt --test dto_fixtures` — 6 + 1 + 8 pass. Log `/tmp/loo291-c2-rust.log`.
- `cargo test -p loopflow --lib -- controller::task run_record::active` — 34 pass (1 ignored cost matrix).
  Clippy then rejected `task_run_spec`'s eighth argument; the redundant `skill`
  argument now derives from the same position. After that change:
  `cargo clippy --all-targets -- -D warnings` passes (`/tmp/loo291-c2-clippy.log`),
  `cargo fmt --all --check` passes, and `controller::task` reruns 24/24
  (`/tmp/loo291-c2-controller-final.log`). `git diff --check` passes.
- `swift test --package-path swift -Xswiftc -gnone --jobs 4 --no-parallel --filter 'SessionRenameTests|DTOFixtureTests|SessionsStoreTests|SessionsPolishTests|WorkspaceNavigationTests|WorkspaceNavigationProofTests|TaskMonitorProofTests|TaskMonitorTests|PodiumModelTests'` — **88 tests / 10 suites pass**. Log `/tmp/loo291-c2-swift-affected.log`.
  - `heldRenameSurvivesNavigationAndPolling`: rename and a pre-rename poll both held; human switches Session mid-flight; canonical name lands, stale poll cannot restore the old name, the other Session is untouched.
  - `rejectionRetainsDraft`: exact text + error retained; leaving the Session drops the unsubmitted draft.
  - `breadcrumbNamesTheExactSession`: one/multiple/Task-only/unmatched trails.
  - `namedSessionDrillDownRetainsTerminal` (real Ghostty, three `/bin/cat` PTYs, fixture registry): row → breadcrumb, rename via pencil/Save with readback, Task crumb → details, row back, sibling switch via final crumb and back; same surfaces, AppKit first responder, unchanged layout, retained draft echoed and companions respond. Rename query called once; any other command fails the test.
  - `sessionFlowMembershipFixture`: all five projections decode, round-trip, missing field rejected.

Counterexamples: removing the generation bump makes the held-rename test fail
(`/tmp/loo291-c2-swift-falsify.log`); restored. First pass of the Flow test
exposed that a historical Run lists only with real provider history (the test now
records one) and that `set_flow_position` needs the current version. Extending
`sessions.json` broke two Podium tests relying on its one-record population →
moved the cases to `session_memberships.json`. Replacing the Task controls first
hid Monitor while a Session was selected (TaskMonitorTests) → Monitor now stays
in the bar whenever the trail has a Task.

Not run: Xcode fallback build, hosted UI, configured provider, live Home. No real
user Session renamed; no PM, install, commit or publication.

## Remaining limitations

- Remote rename/title behavior is **source-only**: remote `human_open_argv` needs
  a HOME-relative worktree, which the temp-worktree fixture cannot provide
  without mutating process HOME.
- Autonomous step Runs record exact membership from now on; existing Runs remain
  `unknown`. A Flow boundary's replacement Run (recovery) is prepared through the
  same `prepare_flow_run`, so it records the same step; carry-over of names is
  cycle 1's `carry_session_name` (Flow-specific replacement not re-exercised).
- `detail` for Flow Sessions still equals the step skill.
- Rename field keyboard focus/Escape are wired but proven only via model/ViewInspector,
  not configured keyboard input.

## Next slice

Slice 3 of the design: replace frame/Wave/Task presentation with accepted A/C —
Flow definition preview and pinned occurrence diagram (membership now available
per Session), three Task situations, comments read, New session beside the title.
