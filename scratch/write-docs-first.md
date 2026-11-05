# LOO-728 — docs first on main

## Accepted direction

Jack Heart requested integration of the saved docs-first implementation on
November 4, 2026. The later independent-main decision (comment
`a084ed83-dc99-446e-9fb4-75a6e0285d78`) supersedes preserving the LOO-498 stack.
Desktop-only automatic presentation remains accepted. Task identity, checkout,
conversation and PR #1817 remain; source PR #1810 and parent PR #1814 stay intact.
Landing is authorized, through the existing code Graph's delivery steps.

Reconciled November 4, 2026: implementation and independent placement exist.
PR #1817 is open against main; the reviewed checkpoint is
`60dd0e3f4141a87f0f556e373b189a76d2e703d7`. Gate passed with the local repairs
recorded below. Publish those repairs, land, then verify delivery.

The complete stacked implementation, prior dirty gate repairs, decisions and
contrary evidence are preserved at
`ea0e0781b22f092b6a0169802997ff7a3628c65c:scratch/`.
Source remains `5e6d3b1fc1ad23bb0fed51f4c0ad9815fed67648`, parent
`df3b5e810d3841a19bc5911c332fb42f23193d46`; none is modified or landed here.

## Experience and owners

`pursue` starts with `write-docs`. Both backward edges return to `implement`.
Write actual README/guide/reference files in place; no scratch documentation
copy, new approval stop or live document synchronization. Implementation reads
saved docs at the start and before finishing, preserving corrections. Unsaved
edits remain the editor's drafts; immediate direction belongs in the conversation.

`lf open --host desktop --file PATH` (repeatable) resolves the selected checkout
and presents originals through the existing Desktop `files(path)` action.
Markdown/HTML use the existing rendered read view. `TaskFilesStore.presentFile`
uses the existing draft and attention policy: active edits retain selection,
other files become tabs. Existing remote Files transport remains the owner.
No new schema, host abstraction, Files store or conversation is created.

Absent Desktop reports paths and opens nothing. Missing paths or unmatched views
report an error; write-docs lists the paths and continues. Recovery retries only
presentation, never docs generation. No terminal/editor/browser fallback.
Explicit Work opening already on main remains unchanged; the source branch's
new terminal `--file` support stays in preserved history with LOO-498.
Universal rendered viewers and remote relative-asset parity are outside scope.

User docs: `README.md`, `docs/index.md`, `docs/orchestration.md`,
`docs/surfaces.md`; `docs/ops.md` explains explicit stack detachment.

## Managed independent placement

`lf sync main` explicitly chooses the default branch for a stacked Task.
Bare sync still follows its live parent. Use the recorded parent fork for the
three-way integration; this removes unchanged inherited work while retaining
child work and the original branch ancestry as recovery evidence. Conflicts
use the existing owned sync/continue/abort lifecycle. A contained main must not
short-circuit the explicit fork comparison. After successful integration, retarget
the existing PR and clear the Task's parent while recording the pinned main SHA.
Failure leaves placement retryable; no direct database writes or branch rename.

Recovery first detached onto `a5000ef889`; the subsequent sync at `582201f799`
integrated main `c59cfcc631`, now the supplied Task comparison base. Upstream
LOO-582 adds explicit adoption of existing work and Session binding. File
presentation continues through the existing Work resolution and Desktop Files
action; it neither adopts work nor binds a conversation. Those upstream changes
need no duplicate ownership mechanism here. The missing-base startup fallback
reports unavailable comparison evidence while leaving the actual checkout
available; the cause of the missing historical base remains unproven.

## Delete — do not maintain

- LOO-498 native workspaces, host locators, draft migration, installer/transport
  changes and inherited Wave-memory edits: remove from this PR's tree; preserve
  in the checkpoint and parent. Existing main behavior is the surviving owner.
- Source terminal file launch routes, shared native editor helpers, current-viewer
  control-state extension and their exclusive fixtures: retained only in source
  history. Desktop original-file presentation survives without those dependencies.
- Blanket rejection of an explicit stacked sync target: default-branch target
  now deliberately detaches using the existing merge/recovery/placement owners.
- Repeated stacked demo reports and inherited parent notes: condensed here and
  in `docs-first-demo.md`; complete evidence remains at the checkpoint above.

The extraction completed those deletions. The surviving Files path uses its
relative-path conversion as the single checkout-containment check. Merge retains
the comparison base from its ancestry check instead of checking it twice; sync
borrows the selected target. No replacement abstraction or new state is needed.

## Remaining work and acceptance

Concept review, November 4: no design change and no unresolved product choice.
Jack Heart's accepted experience maps to three existing owners: checkout files
hold saved documentation, Desktop Files holds drafts and attention, and the
Graph controls execution. Presentation is an action on those files, not a new
document lifecycle, review state, Session bind or approval. The current usage
docs and skill instructions express that contract; keep them unchanged.

Normal path: write the original guide, present it in matching attached Files,
save a correction, then read the saved guide during implementation. On repeat,
active editing retains its draft and selection; another document becomes a tab.
Absent or unmatched Desktop leaves the original files available and work
continues; retry only presentation after correcting the destination. An unsaved
edit being treated as implementation input, an automatic host launch, or a graph
loop regenerating the guide would invalidate this model. Immediate direction
continues through the conversation; live document synchronization is excluded.

Independent delivery changes the integration target, not the Task's identity.
The owned sync records placement after integration and PR retargeting; conflicts
and retarget failures retain recovery through the existing operation. Missing
comparison history remains unknown evidence, never an empty diff. No additional
placement lifecycle is needed. The gate evidence below covers the combined
candidate's affected checks and headless Desktop interactions; this review
does not promote historical passes or headless evidence into mounted UI proof.

Publish the gate repairs, then land and verify the delivered behavior.
Managed detachment and initial publication are complete;
recovery history and its evidence limits are in `recovery.md`.
Recorded focused checks cover absent-Desktop behavior, original paths, no extra Session,
active draft/tab retention, and clean/conflicted detachment preserving parent and
child identities. Gate covered affected graph/progression, world recipes, goldens,
DTO/action fixtures, sync/recovery callers, docs and headless Swift checks.
Relevant Rust modules: `git`, `ops::sync`, `lf::commands::ops`,
`lf::commands::open`, `lf::commands::desktop`, `graph::compiled`, `task_graph`;
integration `sync_tests`, `work_open_tests`; world recipe and prompt binaries.
Git sync callers also reach `agent_startup_tests`, `release_tests` and
`release_lock_tests` in the tests README. The latest main integration changed
Task/Session ownership and transport code; the earlier passes alone did not
establish that the combined candidate passes these affected suites.

Demo/review owns configured attached Files rendering, an active draft surviving
repeat presentation, saving/rereading the correction and conversation continuity.
Earlier headless checks and configured absent-Desktop fallback are supporting
history, not proof for this extracted tree. No display environment is available.
The earlier broad Swift suite exited without an assertion or timeout; focused
Files/pane/DTO and Theme tests passed. That unexplained exit remains historical,
not a passing full suite. No parent landing or installation is required here.

Check: `gh pr view 1817 --json baseRefName,headRefOid,state` confirmed open/main/`582201f799`; prior focused detachment, Desktop-fallback and Task-context tests plus fmt/clippy passed before the latest main sync; no code changed during realign, so no tests rerun; combined affected suites and Swift remain with gate, mounted UI evidence with demo/review.

## Gate evidence — November 4

Reviewed the full diff against the supplied base `c59cfcc631`, including the
existing uncommitted demo notes. The plan maps graph/skill changes to graph,
Task progression, catalog, world recipes and prompt goldens; file presentation
to open/Desktop, Work-opening, DTO and headless Swift checks; sync/recovery to
Git, sync, Task, startup, release locks, release and PR-range/authority checks.
Docs reach CLI spelling, README/index and website checks. No FFI wire change,
terminal implementation, migration, or Xcode build configuration was added.

Two repairs remain in the working tree for delivery. Auto file presentation
previously swallowed an invalid Task selection and returned success as though
Desktop were absent. It now preserves Work-resolution errors, while an
unselected call outside a repository remains a no-op; the regression and
usage docs cover this distinction. The existing root-child sync fixture now
uses `EnvGuard`, since completed sync records placement and must not consult
the installed registry. No product behavior was changed to accommodate it.

Commands below ran with inherited `LF_*` removed and stdin closed. The first
run selected the installed CLI ahead of the candidate for website generation;
the follow-up corrected PATH to `target/debug`, then rustup's proxy directory.
No timeout occurred. Unchanged suites retain their result; the follow-up
covered the repaired inputs rather than repeating the full gate.

- `uv run python scripts/test.py --base c59cfcc631 --reuse-passing --rust-filter 'test(git::tests::) | test(ops::sync::) | test(lf::commands::ops::sync_performance_tests::) | test(lf::commands::open::) | test(lf::commands::desktop::) | test(graph::) | test(task_graph::) | test(ops::task::tests::) | test(sync_tests::) | test(work_open_tests::) | test(agent_startup_tests::) | test(release_lock_tests::) | test(cli_discovery::) | test(documented_commands::) | test(dto_fixtures::) | test(task_graph_launch_tests::) | test(skills::catalog::) | binary(release_tests) | binary(golden_prompt) | test(world_recipes::) | binary(task_pr_range_tests) | binary(task_pr_authority_tests)'`: architecture passed; Rust 503 passed, one fixture failed at the installed-registry boundary; website failed before tests on installed CLI selection; Desktop build, 687 headless Swift tests in 89 suites and Swift boundary check passed (297 s including builds, 136 s test execution).
- `cargo fmt --all`: passed after repairs.
- `uv run python scripts/test.py --base HEAD --reuse-passing --website --rust-filter 'test(lf::commands::open::) | test(work_open_tests::) | test(sync_tests::existing_root_child_syncs_parent_from_its_original_fork)'`: passed in 161 s; 21 Rust tests including both repairs, architecture, formatting, candidate CLI build, `cargo clippy --all-targets --jobs 4 -- -D warnings`, and 72 website tests. Stable rustc 1.99.1 was current after `rustup update stable --no-self-update`.
- `git diff --check`: passed.

Local logs: `/tmp/loo-728-gate.log`, `/tmp/loo-728-gate-repair.log`; the substantive
results are retained above. Resource postflight passed with 149.1 GiB free.
Cleanup retained unrelated builds because the installed schema lacks
`machines.is_local`; no installed schema or parent work was changed. The Swift
linker warned about cached native objects targeting macOS 26.2 versus 26.0,
but app build and tests passed. The earlier unexplained broad Swift exit is
historical; this combined candidate has a completed full headless pass.

CI still owns the full matrix and Xcode app/test-runner compile. Optional native
display diagnostics were skipped, not passed. Installed-database compatibility
and the configured attached-Desktop walkthrough retain the limits documented
in `docs-first-demo.md`; no screenshot or mounted UI evidence is claimed.
