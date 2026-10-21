# LOO-531: Check every Desktop read against a built world

Status: implemented, realigned and gated 2026-10-20; PR #1619 open. CI on
`e1b4766648` failed; the Swift boundary repair and rebase onto #1622/#1623 are
done locally; force-push and the Linux PTY rerun remain. Jack Heart filed this on 2026-10-20 as an
autonomous follow-up to lf-world (#1612). Keep inline JSON for pure view logic,
failure/transitional states, terminal/focus proofs and DTO contract fixtures.

## Problem

Desktop decodes every `lf --json` read through `RegistryQuery`. Today the Swift
side is checked against hand-written JSON under `tests/fixtures/dto/`, which
drifts from what Rust actually emits. A Rust DTO rename or a new required field
that Swift doesn't know reaches Jack as a blank pane, not a failing test.
`WaveDetailReadingTests.populatedDetailHierarchyProjectsThroughLensGrammar`
says populated Wave detail "can't be driven live" and walks `wave_detail.json`
instead. lf-world now makes populated stores cheap, so both gaps can close.

## Demo

```bash
cargo build -p loopflow --bin lf --bin lf-world
scripts/test_desktop.sh --filter WorldReadTests -Xswiftc -gnone   # passes, one row per read
# rename a field in a Rust DTO, e.g. add #[serde(rename = "...")] to a WaveSnapshot field
cargo build -p loopflow --bin lf
scripts/test_desktop.sh --filter WorldReadTests -Xswiftc -gnone   # fails: the "wave status desktop" row, keyNotFound on that field
```

The failing row names the read (`wave status desktop`) and the coding path
Swift could not decode.

## Risk findings (probed 2026-10-20 against a built `portfolio` world)

- **`RegistryQueryLocal` can't be pointed at a world under SwiftPM.** It resolves
  its `lf` via `Bundle.main.url(forAuxiliaryExecutable: "lf")` or the
  `LF_CLONE_ROOT` clone contract; under `swift test` Bundle.main is the xctest
  runner and a world is not a clone (`clone.json`, read-only history). Pointing
  it would mean reshaping production code for tests. **Decision:** the helper
  builds a `RegistryQuery` with the same public read methods over an isolated
  runner. What drifts is decoding in `RegistryQuery`, not `RegistryQueryLocal`'s
  transport (already covered by `WorkObservationTransportTests`). The
  live `monitor work --watch` frame goes through the production
  `LocalWorkObservation.start(process:)` with a world-environment process.
- **Every read runs.** With the built `lf`, all ~35 read argvs exit 0 against
  `portfolio`, including `planning status`, `doctor`, `repo tokens`, `task
  location`, `open --link`, `session bind --dry-run` and `monitor work`.
- **Empty reads prove nothing.** `[]` decodes into any array type. Probed:
  `usage --days 0 --task <id>` returns `[]` for both Tasks with bound Sessions.
  Cause: `World::session` admits the transcript in the repository and binds
  afterwards; prospective attribution (`lf/commands/bind_attribution.rs`) keeps
  usage with its owner at spend time. The world must create attributed history
  for `taskHistory` to check `SessionHistory` populated (see Implementation
  decisions).
- **Chapter preview is a read.** `realignProjects(preview: true)` runs
  `repo new-chapter --dry-run` over stdin. Against `portfolio` it returns the
  desktop predecessor and 2 Tasks and leaves `projects` unchanged (3 → 3). The
  plan JSON needs the desktop `wave_id` (from `allWaves`), a UUID
  `successor_id`, `create: true` and one nonempty KR; `"{}"` or a slug fails.
- **Identifiers are random** (`a45828`). Tests discover Tasks, Sessions and Flow
  IDs from reads by name, never hard-code them.
- **Build cost.** `lf-world build portfolio` takes ~18 s debug on this Mac.
  Build once per test process and share it; reads must not mutate the world.
- **`wave_detail.json` must stay.** `rust/loopflow/tests/dto_fixtures.rs`,
  `DTOFixtureTests`, `RegistryQueryTests`, `WorkModelTests` and three other
  `WaveDetailReadingTests` (failure/transition states, explicit metric
  evidence) use it as a DTO contract fixture, which the Task brief keeps.
  Only the populated-hierarchy test moves.
- **No `lf` in the swift build step for lf-world.** CI (`ci.yml` "Build CLI for
  Swift transport tests") and `scripts/test.py::_swift_commands` build only
  `--bin lf`. Both add `--bin lf-world`.

## Approach

### Shared helper: `swift/LoopflowTests/BuiltLf.swift` (built)

One place owns "run the source-built `lf` in an isolated environment":

- `BuiltLf(home:cwd:overriding:)` — the private Machine environment (PATH,
  HOME, LF_HOME, LF_BIN, LF_USER_NAME, git isolation) formerly duplicated in
  three suites; `overriding` layers a world's printed environment on top.
- `BuiltLf.binary("lf" | "lf-world")` — `LOOPFLOW_TEST_LF` /
  `LOOPFLOW_TEST_LF_WORLD` or `target/debug/<name>`, failing with the
  `cargo build -p loopflow --bin lf --bin lf-world` instruction.
- `run(args, cwd:, input:, executable:)` — file-backed output,
  `terminationHandler`-based per TESTING.md; nonzero exit throws
  `RegistryQueryError` with stderr.
- `query` → `RegistryQuery` with `run`, `runWithInput`, `start` and
  `watchWork` (through `LocalWorkObservation`) wired to the runner.

`World.shared(recipe)` builds each recipe once per test process (actor-cached
`Task`) into `loopflow-world-<recipe>-<pid>` under the temporary directory, decodes the environment
`lf-world build` prints, and removes worlds whose process has exited.
`world.query` and `world.repo` serve the reads.

### Drift check: `swift/LoopflowTests/WorldReadTests.swift`

`@Suite(.serialized)` over the `portfolio` world. A parameterized
`@Test(arguments: WorldRead.all)` calls each public `RegistryQuery` read once and
asserts the decoded value is populated (non-empty collection / expected
member present), so a decode failure or a silently empty shape names its read.

Reads covered: `portfolio`, `repositoryIdentity`, `repositoryPath`, `allWaves`,
`waves(repoPath:)`, `repositoryWaves`, `status(wave:)` for `desktop` and
`systems`, `plan`, `roadmap(nil)`, `roadmap("desktop")`, `workLink` (Task and
Wave addresses), `taskDestination`, `taskLocation`, `peerPlanningStatus`,
`localMachineId`, `userName`, `workActivity`, `automationStatus`,
`taskChanges`, `taskDiff`, `taskFiles`, `taskFile`, `taskFlowProcesses`,
`flowCatalog`, `workflowCatalog`, `workflowSource` (a listed workflow),
`taskComments`, `taskHistory`, `taskContext`, `sessionPage` (both modes),
`previewSessionBinding` (untracked Session → unstarted Task),
`realignProjects(preview: true)` (desktop successor plan), `usage`,
`codebase`, `codebaseHistory`, `doctor`, plus one `monitor work --watch` frame
through `LocalWorkObservation`.

**Coverage is structural.** A second test reads `RegistryQuery.swift` via
`#filePath`, lists its `public func` names, and requires each to appear in
either `WorldRead.all` or `WorldRead.excluded` (name → reason: mutation,
starts work, stream covered elsewhere, composition helper such as `onMachine`
/ `recording` / `pinTaskLocation`, `decode`). Adding a read without a world
check fails the suite with the method's name.

Excluded with reasons (`WorldRead.excluded`): writes (`ensureProject`,
`setAutomation`, `automateTask`, `saveWorkflow`, `customizeFlow`, `moveTask`,
`pause/complete/reopen/abandonTask`, `updateTaskDirective`, `saveTaskFile`,
`bindSession`, `renameSession`, `resetSession`, `ensurePrimarySession`,
`prepareTaskSession`, `prepareSession`); starts work (`runTask`,
`resumeInterrupted`); `prepareSessionAttachment` (nil without a running
Session); `watchTaskFiles` (covered by `TaskFilesTests`); composition helpers
(`onMachine`, `pinTaskLocation`, `recording`) and `decode`. `watchWork` is a
world row.

### Populated Wave detail from the world

Rewrite `populatedDetailHierarchyProjectsThroughLensGrammar` to
`world.query.status(wave: "desktop")`: objective non-empty, current Project
carries the recipe's two KRs by text, open-Task count equals the recipe's open
desktop Tasks, every Task lens equals `WaveLensColor(condition.state)` with its
reason verbatim and non-empty, and the Project lens derives from those Tasks.
Delete the "can't be driven live" comment. The fixture-specific
`INF-123`/merge-head assertions go; the shared lens grammar stays proven on
real Rust output. Other three tests keep `wave_detail.json`.

## Scope

In: helper, drift check, coverage test, populated-detail rewrite, recipe
attribution fix, swift build steps building `lf-world`, TESTING.md lines.

Out: reads outside `RegistryQuery` (none exist on macOS today; `queryLf` has no
other callers); mutation round-trips; UI tests (`WaveSurfaceLaunch` already
uses worlds); replacing DTO contract fixtures; the known "completed Task
lists under Started" and AttributeGraph-cycle bugs from #1612.

## Delete — do not maintain

- `WorkOpeningCLITests.Fixture` binary resolution, env and `run` → `BuiltLf`.
  Its repository setup stays (it needs two same-named repos, not a world).
- `WorkCLIFixture` binary resolution and env dictionary → `BuiltLf`; its
  file-handle streaming runner stays (it observes a long-lived watch).
- `PortfolioServiceTests.desktopWritesReadBackThroughCLI` env gate
  `LOOPFLOW_PORTFOLIO_TEST_BIN` and its inline runner → `BuiltLf.query`; the
  test now runs in the suite. Drop the gate line from TESTING.md:1535.
- The "can't be driven live" comment and the `wave_detail.json` dependency of
  the populated-hierarchy test.

Forbidden: a second lf-resolution path in tests; a world-mode switch inside
`RegistryQueryLocal`/`LocalWaveAgentLauncher`; per-test world builds; hand-written
JSON standing in for a populated read.

## Implementation decisions (2026-10-20)

- **Attribution:** binding is prospective, so checkout `cwd` alone never
  attributes. `World::session` writes the transcript in the Task's checkout,
  binds, then takes the Session's next input through production `append_input`
  (now `pub(crate)`), as reconnecting a bound Session does.
- **Populated reads:** `portfolio` also leaves an uncommitted README edit and a
  comment on the running Task (`World::edit`, `World::comment` via
  `task_comment`), so diff, changes, file and comment rows read members.
- **`config user`:** a world writes `user.name: Demo` to its `config.yaml`; the
  same `PERSON` makes its commits. Without it the read is `null`.
- **Real drift found:** Desktop's chapter preview decoded
  `tasks[].task.name`; Rust emits `title`. Swift `ProjectRotationPreview.Issue`
  now reads `title` and the view and `ProjectActivationTests` follow.
- `watchWork` is a world row requiring every Work part with a body.
- `WorkCLIFixture`'s runner also moved onto `BuiltLf.run` (file-backed,
  `terminationHandler`), and its tests read through `BuiltLf.query` instead of
  hand-wiring the same `RegistryQuery`.

## Remaining work: CI repair (kickoff 2026-10-20)

PR #1619 run 39509447006 failed `swift-test`, `rust-test (integration, 1, 3)`
and the `tests-result` aggregate. Base `051a8561` passed on main.

- **`swift-test` — caused by this branch.** `scripts/check_swift_multiplatform_boundaries.py`
  rejects `WaveDetailReadingTests.swift: new non-shell #if`. The world-backed
  populated-hierarchy test sits inside an inline `#if os(macOS)` because
  `World` lives in whole-file-gated `BuiltLf.swift`. The checker accepts only
  whole-file gates outside app shells.
  **Done (implement 2026-10-20):** moved `populatedDetailHierarchyProjectsThroughLensGrammar`
  unchanged into `WorldReadTests.swift`, which is already whole-file gated and
  already owns the shared `portfolio` world. Drop the inline gate.
  `WaveDetailReadingTests` keeps its three `wave_detail.json` tests and
  `loadFixtureData`. Rejected: gating all of `WaveDetailReadingTests.swift`,
  because it would hide fixture tests that don't need a world. Rejected:
  `#else` fallbacks, because they add a branch that never runs.
  Focused check: `uv run python scripts/check_swift_multiplatform_boundaries.py`
  passes, and `scripts/test_desktop.sh --filter 'WorldReadTests|WaveDetailReadingTests' -Xswiftc -gnone` passes.
- **Rebase onto main — done (implement 2026-10-20).** Squashed to one commit
  (`e2d1145c02` on `2ed7a95ea7`; pre-squash history at local branch
  `loo531-pre-squash-backup`). Took main's `world/mod.rs` and reapplied only
  `PERSON`/`config.yaml`, `World::comment`, and bind → `append_input` →
  `write_provider_session`; `World::workdir`/`edit` gave way to main's `cwd`/
  `uncommitted`. The Task-history assertion now runs over every recipe through
  `Built::lf`. MEMORY.md keeps main's text (the branch carried only compaction).
  Force-push of the rebased head is owed to publish.
- **`rust-test (integration, 1, 3)` — not traced to this diff; remaining.**
  `session_launch_contract::loopflow_warnings_preserve_native_drafts_across_providers`
  (a standalone PTY binary) failed on Linux at `session_launch_contract.rs:141`
  with `EDITOR_EXIT` in the output. This branch touches neither that test nor its
  launch path; the only nearby change is making `append_input` `pub(crate)`.
  Rerun the job after the Swift fix. If it fails again on the new head, run
  that test on main locally (Linux CI only; main pushes skip integration jobs,
  so no main run exists to compare) before changing anything. The PTY
  draft path belongs to LOO-508, not this Task.

## Done when

- `scripts/test_desktop.sh -Xswiftc -gnone` runs `WorldReadTests` (no env gate,
  no skip) after `cargo build -p loopflow --bin lf --bin lf-world`; every read
  row passes with populated data.
- A deliberately renamed Rust DTO field (e.g. a `WaveSnapshot` field) makes
  the matching row fail naming the field; reverting passes. Gate runs this probe
  once and records the line.
- Adding a `public func` to `RegistryQuery` without a world row or exclusion
  fails the coverage test.
- `WaveDetailReadingTests` no longer claims populated detail can't be driven
  live, and that test reads the world.
- `cargo test -p loopflow --test integration world_recipes` passes with Task-attributed
  history (`lf usage --days 0 --task` non-empty for a Task with a Session).

## Checks

- kickoff 2026-10-20: `repo new-chapter --dry-run` on `portfolio` — desktop predecessor + 2 Tasks, no Project written.
- kickoff 2026-10-20: `lf-world build portfolio` + 35 read argvs with built `lf` — all exit 0; Task-scoped `usage` empty (recipe gap above).
- implement 2026-10-20: `scripts/test_desktop.sh --filter 'WorldReadTests|ProjectActivationTests|WaveDetailReadingTests|PortfolioServiceTests|WorkOpeningCLITests|WorkCLISubscriptionTests'` — pass; `world_recipes` pass; clippy/fmt clean.
- implement 2026-10-20: probe `#[serde(rename = "active_task_count")]` on `WaveSnapshot.active_tasks` — 22 rows fail with `keyNotFound "active_tasks"` naming their read (Task rows find their Task through `wave show`); reverted. Full Swift suite owed to gate.
- compress 2026-10-20: `scripts/test_desktop.sh --filter WorkCLISubscriptionTests` — pass after moving its three queries onto `BuiltLf.query`.
- realign 2026-10-20: plan helper/exclusion sections rewritten to the shipped `BuiltLf`/`World`/`WorldRead.excluded`; no code change, no rerun.
- publish 2026-10-20: `scripts/test_desktop.sh -Xswiftc -gnone` (full) — 616 tests pass incl. 40 world rows + coverage; `world_recipes`, fmt, clippy pass. Rename probe from implement stands; CI owns hosted run.
- gate 2026-10-20: rebased onto #1614 (world stub `claude`); `world_sessions_resume_live_through_the_stub_provider` failed because the Task Session's appended input carried no native conversation and connect started fresh. `World::session` now records the provider conversation on that input. `world_` integration tests (2) pass, fmt/clippy clean, `pytest test_desktop_performance.py` 51 pass, `scripts/test_desktop.sh --filter 'WorldRead|ProjectActivation|WaveDetailReading|PortfolioService|WorkOpeningCLI|WorkCLISubscription|DesktopPerformance|DesktopNativeSession'` 37 tests pass (40 world rows). CI owns the full matrix.
- kickoff 2026-10-20: CI run 39509447006 on `e1b4766648` — swift boundary check fails on the inline `#if`; Linux PTY `session_launch_contract` fails, not traced to the diff.
- implement 2026-10-20: moved populated-hierarchy test into `WorldReadTests`; boundary checker passes; `scripts/test_desktop.sh --filter 'WorldReadTests|WaveDetailReadingTests' -Xswiftc -gnone` 10 tests (40 world rows) pass. PTY rerun owed to CI after push.
- compress 2026-10-20: shared `World.wave(named:)` for the two Wave-id lookups; `scripts/test_desktop.sh --filter WorldReadTests -Xswiftc -gnone` 3 tests (40 rows) pass.
- kickoff 2026-10-20: `git merge-tree HEAD origin/main` — conflicts with #1622 in world/mod.rs, world_recipes.rs, MEMORY.md; resolution in Remaining work.
- implement 2026-10-20: rebased onto `2ed7a95ea7` — `cargo test -p loopflow --test integration world_` 2 pass (Task history over all 5 recipes); `scripts/test_desktop.sh --filter 'WorldReadTests|WaveDetailReadingTests' -Xswiftc -gnone` 10 tests (40 rows) pass; boundary checker, fmt, clippy clean.
- compress 2026-10-20: `World.build` takes the repo from the directory it passed `lf-world` rather than deriving it from printed `LF_HOME`; `scripts/test_desktop.sh --filter WorldReadTests -Xswiftc -gnone` 3 tests (40 rows) pass.
