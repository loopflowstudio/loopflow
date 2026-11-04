# LOO-728 — integrate write-docs-first

Jack Heart requested integration on November 4, 2026, preserving the source
branch, checkout, conversation and draft PR #1810 at
`5e6d3b1fc1ad23bb0fed51f4c0ad9815fed67648`. LOO-728 remains stacked on LOO-498
(PR #1814); its own PR #1817 is open. Loopflow merge `0cb4421cb8` integrates
published parent `df3b5e810d3841a19bc5911c332fb42f23193d46`, including the
coherent installer and revised draft workspace schema.
This records integration, not parent acceptance or authorization to land it.

## Accepted experience and owners

Jack Heart's November 3 direction: explore through prototypes and diagrams,
then write the actual user docs before implementation. Review and editing remain
available while work continues, with no extra approval stop or scratch docs copy.
Jack chose user documentation, not a press release. Sources:
[user-docs-first](https://blog.larah.me/write-the-docs-first/) and
[working backward](https://maven.com/articles/start-with-the-press-release-amazon-method).

`pursue` begins at `write-docs`; both backward edges return to `implement`.
Implementation reads saved docs initially and before finishing, preserving
corrections. Unsaved drafts belong to the editor. Immediate direction belongs
in the Task conversation; no live synchronization is promised.

Jack Heart accepted Desktop-only automatic presentation on November 4 in
comment `50456e16-156c-424e-8fa1-67030496483f`, superseding automatic terminal
presentation. `write-docs` uses `lf open --host desktop --file PATH` for the
original files. An already attached Desktop Files view renders Markdown/HTML
and preserves its draft/attention policy. Absent, unavailable or unmatched
Desktop means report the actual paths and continue, without a terminal/editor,
browser, host or conversation fallback. This changes workflow presentation only.
No universal rendered viewer or new approval stop is included.

Explicit `lf open --file PATH`, repeatable, still presents the original files through
`open::files::run` without starting a host or conversation:

- Desktop's `files(path)` action uses `TaskFilesStore::presentFile` and the
  existing attention policy. Active edits stay selected; other files become tabs.
- Native hosts create adjacent editor surfaces using `task_tool::file_editor`.
  Rex reuses an existing file pane. Native editors require a local checkout;
  Desktop retains its remote Files transport.
- Transport's `control_state` supplies the current controlling viewer's
  placement through the existing Session summary. Passive viewers cannot redirect
  presentation; detached or unknown owners supply no destination. Direct native
  Sessions retain recorded placement. No new Session, Task state or schema is added.

With no attached host, presentation is a successful no-op. Missing files and
failed native requests report failure without changing docs. Repair the path or
host and present those files again; do not rerun docs writing to recover. Inspect
panes after partial multi-file success before retrying; cross-host deduplication
is not promised. Preserve original files, drafts, panes and Session identity.

Affected user docs: `README.md`, `docs/index.md`, `docs/orchestration.md`, and
`docs/surfaces.md`. The parent's `docs/workspaces.md` retains its explicit gaps.

## Delete — do not maintain

Completed cuts and their surviving owners:

- Inline Files editor selection and local `viewer_argv(task tool)` launches →
  shared `task_tool::file_editor`; no installed-CLI override for presentation.
- Execution-process placement and boolean-only `transport::controlled` →
  controlling-viewer placement from `control_state` and the Session summary.
- Repeated viewer reads, per-file Desktop setup and duplicate native traversal →
  one Desktop batch/pane and shared Rex traversal. Reports borrow paths;
  canonical paths are for comparison, recorded checkout spelling for execution.

The superseded automatic current-host command and its terminal-demo acceptance
requirement are removed from the skill, user docs and active plan. Explicit
terminal file opening and its regression tests remain required behavior.
No code predecessor or exclusive fixture remains to delete. The current diff
review found no further worthwhile code reduction. Repeated demo instructions
and diagnoses now live once in `docs-first-demo.md`; superseded iteration
narratives are retained in Git. Add no competing Files store, viewer registry,
host launcher or persisted review lifecycle.

## Remaining delivery and acceptance

November 4 integration synchronized published parent
`df3b5e810d3841a19bc5911c332fb42f23193d46` through `lf sync --manual`.
Local contributions were checkpointed first. The parent's coherent installer,
finished draft `rex_workspaces` schema, native locators/creation receipts and
append-only follower are retained. Presentation helpers continue to use current
viewer placement and the shared editor policy; they add no host-record authority.
The surfaces conflict removes the superseded duplicate workspace guide while
retaining original-file presentation. `docs/workspaces.md` owns workspace behavior.
Parent scratch is preserved at that revision; this checkout retains concise
LOO-728 notes and its own concept review. Focused verification is recorded below.

Parent delivery still precedes supported shared-store promotion; its unmerged
migration cannot be bypassed or landed under LOO-728's approval. The coherent
installer is integrated but not installed. Source and parent PRs remain intact.
Configured terminal auto-presentation is no longer an acceptance prerequisite;
its failed attempts and bounded direct/native successes remain historical evidence
in `docs-first-demo.md`.

Remaining demo/review proof follows the accepted Desktop-only workflow: present
original Markdown/HTML in an already attached Files view, preserve an active draft
on repeat, save and read the correction from the same original, recover a failed
presentation, and continue the same conversation. Outside attached Desktop,
report paths and keep working without launching anything. Headless checks cover
rendering/drafts and no-Desktop behavior; mounted/configured proof belongs to the
primary conversation when a compatible installation is available. Existing
Sessions, provider history, source work and recovery bytes remain preserved.

## Direction and dependency boundaries

The primary conversation accepted the Desktop-only choice; there is no unresolved
presentation-policy blocker. `rendered-file-viewers.md` retains research and
`concept-review.html` records the accepted choice. Explicit terminal editing
remains distinct from rendered viewing. Desktop's macOS client and unproved
remote relative assets establish no universal renderer.

Jack Heart's November 4 parent decision assigns native restoration and shells
to Rex: LF launches templates, preserves retained destinations and never closes
Rex shells. Retained Task Session terminals prevent cleanup regardless of focus;
confirmed final removal releases that hold, while unknown inventory does not.
This supersedes the inherited full-restoration and Rex-settlement requirements,
without changing the separate Desktop-only docs-workflow decision.
LOO-498 still owns native split, shared retention integration, remote routing
and configured reuse proof. `rex-workspace.md` records the revised dependency;
`workspace-demo.md` retains dated evidence. LOO-728 preserves source PR #1810, parent PR #1814 and
their work; parent delivery remains outside this step. Task inventory identifies
interactive Session `fd5f2333c5f54054b40c4bb5e574a98a` in this checkout;
availability alone proves no Rex placement. No Task/Wave planning changes follow.

The accidental `docs/.orchestration.md.swp` stays excluded from delivery and
ignored, with local recovery bytes preserved. Prior iteration feedback restored
the parent's November 3 Program Status from `b8aac2ef13` verbatim; see
`questions.md` for attribution and integration assumptions.

## Validation

Earlier integration evidence (before the new parent sync): graph compilation/Task progression (13 tests),
two changed world scenarios, prompt goldens, action DTO and README/index sync
passed. Headless Desktop passed 55 TaskFiles/pane/DTO tests; optional display
diagnostics were skipped. The isolated real-Rex candidate probe passed opening,
draft preservation, saved-file readback and missing-file recovery. Jack Heart's
“ok it opened” confirms direct development CLI opening. Neither proves configured
agent-route acceptance; [docs-first-dev-demo.md](docs-first-dev-demo.md) retains
that demo, screenshots and its limits.

Check (implement, November 4): `cargo build -p loopflow --bin lf`, migration-history check, fmt, and `cargo clippy --all-targets -- -D warnings` pass; `cargo test -p loopflow --test integration work_open_tests:: -- --test-threads=1` passed 20 checks and skipped 2 native probes, with one flaky mock-argv fixture removed after its 2s timeout; the focused `work_open_tests::desktop_file_presentation_without_desktop_never_falls_back_to_a_terminal -- --exact` rerun passes after cleanup; `uv run --project website --extra test pytest website/tests/test_readme_index_sync.py`, `git diff --check`, and `lf context` pass. Existing unchanged Files rendering/draft checks remain applicable; gate owns broader affected suites and demo/review owns configured Desktop experience after compatible installation.

Historical plan/checks: `13e7d5dac8:scratch/write-docs-first.md`; publication:
`c136addfad33b65a4080f609f12da2c1a663b85c:scratch/write-docs-first.md`.
Pre-compression working plan, including prior unpublished reconciliation, is
preserved as Git blob `84005cbc45e3a58810ae609439fd87007c550fe4` (read with
`git show <blob>`). Its earlier archive is `7234a2e609b71e189e0020f19bcc9817441c89b5`.
