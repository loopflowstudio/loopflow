# Cycle 3 — implement: calmer frame, Wave plan, basic Task overview

2026-09-25. Bounded slice of `scratch/main-view-task.md`. No commit, publication,
install, PM write, store migration or live Session operation.

## Changed behavior

- **Shared started-work fact.** `TaskRuntimeSnapshot.started: bool` (Rust + Swift,
  required, no default). `Store::task_started` reads existing evidence only:
  a `started`, `progress`, `body_handed_off` or `flow_finished` Task event, or a
  Flow position with `worker_generation > 0`; the roadmap adds any published or
  merged PR. Unlike chapter `begun`, a prepared human `session_run_id` does not
  count. One indexed query per Task per refresh, alongside the existing
  `task_prs`/`task_execution` reads — no Run-history scan, no new writer or
  registry. `false` means no recorded evidence; the doc comment says so.
- **Sidebar.** Burgundy repository header (serif name, switcher menu, presentation
  menu) heads the deep-wine outline; search moved to the bottom. Repository rows
  removed from the list. Sidebar Tasks = started and incomplete, or any Task with
  open Sessions (keeps Sessions reachable when start predates recorded evidence).
  Upcoming/prepared-only Tasks appear only in the Wave plan; inspecting never
  adds them. Task Session leaves replaced by an inline count (bubble + n) whose
  help/accessibility names each Session; clicking the count inspects the
  overview; the row context menu opens each Session by name. Wave-level and
  unmatched Sessions remain leaves; the flat Sessions presentation still lists
  every Session. ELSEWHERE/VIEWING/RUNNING sidebar badges deleted
  (`sessionRowStatus`, `SessionItem.statusLabel` and five tests); the in-pane
  Move here explanation is unchanged.
- **Task selection.** Exactly one open Session → `openSession` (existing native
  surface, focus, draft); zero or several → Task overview. Monitor stays an
  explicit breadcrumb control. Saved-pane restoration on Task click is no longer
  the entry path (`taskPanes` is still written by `rememberTaskPane`; see below).
- **Wave page.** Serif Wave title, paused chip and existing Home control;
  objective; `Current KRs` (explicit "No current chapter plan."); full ranked
  Task plan as calm rows (click follows the Task rule); partial/unavailable
  warnings; out-of-plan Tasks; metrics; Chapter history link last.
- **Task overview.** Breadcrumb `Wave / ID` (ID links to Linear); title once with
  **New session** beside it; condition reason with existing legal Task controls;
  `Sessions · n` list (exact names, membership, state) or "No open Sessions."
  only after a successful Session read; `Description` rendered from source
  Markdown with **Edit description** (existing PM editor, relabelled). Removed:
  planning-snapshot line, chips, worktree path, chapter block, Inspect/Sessions
  menu in the breadcrumb. In a Session the breadcrumb is Wave / Task title + ID /
  Session name (unchanged cycle-2 behavior).
- **New session.** Uses the existing scoped conversation launch
  (`lf --interactive --task <ID> : …`) in the Task's checkout. If no local
  checkout exists, first runs `lf task prepare <ID> --json` (existing, no worker,
  no Flow) and launches in the returned `worktree`. Errors appear in the Task
  surface's control banner.
- Deleted the `activeWorktrees` projection input: a live process in a checkout no
  longer affects membership.

## Proof

Commands cleared ambient `LF_*` Home/Run/Task/Flow/Session variables for Rust.

- `cargo test -p loopflow --test flow_tests observing_and_preparing_a_task_are_not_execution`
  — 1 pass (`/tmp/loo291-c3-started.log`). Extended to assert `task_started`:
  false for a prepared unrun Task, false after a filtered active-Run read and an
  unopened human review (prepared Run), true after a real independent Run
  (owned codex stand-in, isolated Home).
- `cargo test -p loopflow --test dto_fixtures` — 8 pass; `--lib lf::commands::waves` — 9 pass.
- `cargo clippy --all-targets -- -D warnings` and `cargo fmt --all --check` pass.
- Swift, one final run: `swift test --package-path swift -Xswiftc -gnone --jobs 4 --no-parallel
  --filter 'WorkspaceNavigationTests|SessionsPolishTests|TaskDirectiveEditorTests|DTOFixtureTests|SessionsStoreTests|PodiumModelTests|RegistryQueryTests|SessionRenameTests|WorkspaceNavigationProofTests|TaskMonitorTests|TaskMonitorProofTests|WaveDetailReadingTests|RoadmapViewTests|LocalWaveAgentLauncherTests'`
  — **125 tests / 15 suites pass** (`/tmp/loo291-c3-swift-final.log`);
  `LocalWaveAgentLauncherTests` 9 pass after adding the prepare-receipt test.
  - `startedWorkingSet`: started/prepared/upcoming/completed fixture → sidebar is
    exactly the started incomplete Task; inspection and Session resolution do not
    change it; all four remain in the Wave plan.
  - `denseWorkingSet`: 3 Waves × 50 Tasks, 20 started → 20 sidebar Task rows,
    3 Waves, all 150 reachable; identical re-read yields identical rows.
  - `namedSessionDrillDownRetainsTerminal` (real Ghostty, three `/bin/cat` PTYs):
    Task row with two Sessions → overview (no Session selected, terminal not
    first responder, `breadcrumb-issue` = W2-131) → `task-session-first` → exact
    Session and native focus → rename → Task ancestor → overview → back; draft and
    companions reply, layout unchanged.
  - `mixedMonitorRetainsInput` (real PTYs): Task row with one Session enters it
    directly; a started Task without Sessions opens its overview with shells
    retained; Monitor split/zoom/return unchanged.
  - DTO: Swift fixture decodes `[false, true, nil, true]`; Rust round-trip fixture carries both values.
- Native captures (fixture reads, real PTYs, hidden window) in
  [cycle-03-evidence](cycle-03-evidence/): `Full hierarchy-false.png` (Task
  overview) and `wave-false.png` (Wave page). Visually inspected. Fixture
  composition only — not configured acceptance or a performance endpoint. In the
  Wave capture the sidebar shows the flat Sessions presentation because the
  capture loop last selected it.

Counterexamples during work: the fixture edit first mis-indented JSON (reverted
and redone); Task-description tests assumed plain text (Markdown strips inline
code backticks; test now compares rendered characters); Markdown paragraph joins
changed from spaces to newlines to keep authored line structure; native tests
clicked Task Session leaves that no longer exist and were moved to the Task row
and overview list; the scroll proof's 80 tasks had no runtime and dropped out of
the started sidebar, so they now carry started evidence.

Source SHA-256 (final):

```text
store/sqlite/chapters.rs   5f9611eda56027c7b942225bb0425779f00d365a2ca2f6d5d449caffe17ca086
store/chapters.rs          da0d3f31f4e55fcee5f45463c87af61ce07265dff4d26e25b5a8a85274d6f00f
lf/commands/waves.rs       2059307583a7624d753b9726bf5ecf61a09f0cde6eb5082ef68427c5f4fa5ddd
tests/flow_tests.rs        7e51891a65453a7df5e897f65f58837b07ed9eb9352e789af03f0bbb56aa14b1
WaveWorkMap.swift          f5515743a77e907395a212d3697707351f953e60e87c7196c7a4045061ffdc3d
WorkspaceProjection.swift  2066a5cc58cece20b214dbae6427156da85f5f4060ecf04761068bfa5be8408b
WorkspaceNavigator.swift   8852289b61c5bae1edb19b805c2667a3d729eb795cce18fc37d67d58cdd71521
WorkSurfaceView.swift      9d0cf10a76bce5e3403665132ecd82c525acd6c2e197f9cc1125818946f63e9a
WorkspaceBreadcrumbBar.swift b09d763b746beedf0d1810204bb0658e3662acebf44ec3f4987404e46a614c96
SessionsView.swift         7e06c24d6f09d761290920321b3361d477a2706770816d7812e8510b83bbfe7e
MarkdownBlocks.swift       73cc3b34c61a7b9fe8e5c6d1754a955dcf65c01d23ddb9ce5507c585f462ea30
MacLocalWaveAgentLauncher.swift befde4c67b70001408de72e3bc2dae4e8fc8a34076da66cd9d129e1194022bc0
PodiumModel.swift          8fe8d4cb9d71a3d10d604b56df132082b263bc5e07c4a17d6c61f3d22fc811b7
roadmap_snapshot.json      614337eaedbd457e14c8e608e455164a7a4232d8ab76014d75d89e11d5725e8d
```

Not run: Xcode fallback build (new file `MarkdownBlocks.swift`; check that
`project.yml` sources glob picks it up), hosted UI, the opt-in desktop benchmark
(readiness probes adapted and compiled only; `monitor_empty` now includes an
inspection lookup inside its measured action, so prior baselines are not
comparable), configured app, real `lf task prepare` launch from the app.

## Owner and deletion review

One reader (Podium), one projection, one navigation owner, existing native
surface pool and multiplexer. The started fact is a read-only projection of the
existing Task event/Flow position/PR owners; no second start registry. Deleted:
sidebar status badges, repository outline rows, `activeWorktrees` membership,
Task Session leaves, Task-overview snapshot/chips/worktree/chapter blocks,
breadcrumb Inspect/Sessions menu.

## Findings for compress/review

- `MarkdownBlocks` hand-parses block prefixes. The parent observation
  (`markdown-rendering-observation.md`) shows Foundation's full Markdown parse
  already yields presentation intents; replacing the hand grammar is a real
  compression candidate.
- `navigation.taskPanes`/`rememberTaskPane` are now written but no longer read by
  Task selection; decide whether Monitor/pane restoration keeps a reader or delete.
- `openTask` counts Sessions from the projection; a Task whose Sessions are
  unavailable (failed read) opens the overview, which says nothing about Sessions
  — correct, but the overview could name the unavailable read.
- Task rows use a `minus` glyph; the accepted reference shows no leading glyph.
- `WaveChapterView` still prints "Chapter <id>" above the KRs (internal copy).
- Historical started evidence before the Started event (runs without a Task
  event, unpublished authored commits) is not recovered; such Tasks appear in the
  sidebar only with open Sessions.

## Remaining full-design scope

Real Flow catalog search/preview and pinned occurrence diagram with both return
edges and final Advance → queue → land; running/paused/blocked situations;
Stop & restart/pause controls; actual comment count/thread; configured demo,
measurements, external trials and authorized edit.
