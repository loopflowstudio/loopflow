# LOO-728 — docs first on main

## Accepted direction

Jack Heart requested integration of the saved docs-first implementation on
November 4, 2026. The later independent-main decision (comment
`a084ed83-dc99-446e-9fb4-75a6e0285d78`) supersedes preserving the LOO-498 stack.
Desktop-only automatic presentation remains accepted. Task identity, checkout,
conversation and PR #1817 remain; source PR #1810 and parent PR #1814 stay intact.
Landing is authorized, through the existing code Graph's delivery steps.

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

Publish the local independent-main commits through the primary conversation's
delivery steps. Managed placement and PR #1817 now target main; recovery details
and the installed/main schema boundary are in `recovery.md`.
Focused checks cover absent-Desktop behavior, original paths, no extra Session,
active draft/tab retention, and clean/conflicted detachment preserving parent and
child identities. Gate owns affected graph/progression, world recipes, goldens,
DTO/action fixtures, sync/recovery callers, docs and headless Swift checks.
Relevant Rust modules: `git`, `ops::sync`, `lf::commands::ops`,
`lf::commands::open`, `lf::commands::desktop`, `graph::compiled`, `task_graph`;
integration `sync_tests`, `work_open_tests`; world recipe and prompt binaries.
Git sync callers also reach startup/release boundaries in the tests README.

Demo/review owns configured attached Files rendering, an active draft surviving
repeat presentation, saving/rereading the correction and conversation continuity.
Earlier headless checks and configured absent-Desktop fallback are supporting
history, not proof for this extracted tree. No display environment is available.
The earlier broad Swift suite exited without an assertion or timeout; focused
Files/pane/DTO and Theme tests passed. That unexplained exit remains historical,
not a passing full suite. No parent landing or installation is required here.

Check: `cargo test -p loopflow --test integration` with the separate filters `sync_tests::explicit_main_sync_detaches_only_the_child_and_preserves_its_pr` and `work_open_tests::desktop_file_presentation_without_desktop_never_falls_back_to_a_terminal` passed after compression, with inherited LF_* unset and stdin closed; `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` passed. The earlier focused Task-context pass still applies to unchanged code. Gate owns affected suites and Swift; demo owns mounted UI evidence.
