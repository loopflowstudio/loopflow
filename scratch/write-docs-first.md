# LOO-728 — integrate write-docs-first

Accepted integration — Jack Heart, 2026-11-04. Incorporate the existing source
at `5e6d3b1fc1ad23bb0fed51f4c0ad9815fed67648`, preserving its branch,
checkout, conversation and draft PR #1810. This Task stays stacked on LOO-498
(PR #1814), whose head was `da41d2031f3526df7fab53292b936dcccc699d9b`
and whose PR remained open at integration. The source merged without conflicts;
the parent's newer workspace guide and corrections remain intact.

Accepted direction — Jack Heart, 2026-11-03: exploration uses prototypes and
diagrams to choose a direction. A new write-docs step begins pursue, before
implementation. Review and editing remain available while implementation continues.

Jack Heart subsequently clarified: edit the actual documentation in place and
ask the current host to open the relevant files. No scratch draft or wrapper.
With no host, presentation does nothing. Jack Heart approved stacking on LOO-498,
whose shared Files tool supplies Desktop's native view and terminal editors.

Implementation choice: `lf open --file PATH`, repeatable, presents files without
opening a conversation or launching a host. Desktop reuses the scratch-follow
attention policy for any file; terminal hosts use LOO-498's editor policy.
Graph loops still return to implement, preserving saved documentation edits.
LOO-498's live work remains separate. Its restoration, native split, lifecycle
and remote-host obligations are not acceptance work for this integration.

Review: a configured-host walkthrough remains required and unverified. Native terminal file
presentation requires a local checkout; Desktop retains its remote Files
transport. LOO-498's native restoration and remote-host gaps remain with that Task.

Sources: [Mark Larah's user-docs-first pattern](https://blog.larah.me/write-the-docs-first/)
and [working backward from the finished experience](https://maven.com/articles/start-with-the-press-release-amazon-method).
Jack Heart explicitly chose user documentation, not a press release.

## Delete — do not maintain

The source extracts `task_tool::file_editor` from the inline Files branch; both
Task and taskless presentation use that policy. No competing Files store,
scratch documentation copy, host launcher or review approval step is added.
Removed repeated viewer reads and per-file Desktop connection/context resolution;
Desktop presents a batch to one selected pane. Rex's caller validation and file
presentation share native block traversal. Reports borrow paths instead of copying
them. Keep recorded checkout spelling for tool execution; canonical paths serve
comparison only (the macOS alias regression was caught by the existing Rex test).
No further predecessor is identified. Preserve original files, active drafts,
existing panes, Session identity and the loop-back-to-implement behavior.

Inherited parent plans/demo evidence are condensed with exact Git references in
`rex-workspace.md`, `workspace-demo.md` and `questions.md`; they grant no work or
scope reduction here. The parent checkout and its original artifacts remain intact.

## Remaining work

Concept review, 2026-11-04: **no design change and no open LOO-728 product
choice**. Jack Heart's accepted interaction remains coherent across the docs,
skills, Graph and Files implementation. This is a design conclusion, not
configured-host acceptance. The existing usage examples and skill instructions
already describe it; no replacement draft is needed.

The concrete interaction is `lf open --file docs/usage.md --file README.md`
after editing those original files. Jack Heart can save corrections while
implementation continues. The next implement pass reads them, and its final
reread catches saved changes during that pass. Before this change, `pursue`
began at implementation; now it begins at `write-docs`, with both backward
edges still targeting `implement`. There is no second document, approval
record or documentation lifecycle to reconcile.

Actions map to existing owners: editing changes checkout files; presentation
uses `open::files::run`, Desktop's `files(path)` action and
`TaskFilesStore::presentFile`, or the native host's pane/tab operation and
shared `task_tool::file_editor` policy. Files owns Desktop drafts and attention;
terminal hosts own their editors and panes. Presentation adds no Session,
Task state or persistence schema. The distinction that matters is saved file
content versus an editor's unsaved draft: implementation consumes the former,
while presentation must preserve the latter. Immediate direction still belongs
in the Task conversation; this introduces no live synchronization promise.

Failure and recovery stay within that model. With no attached host, the files
remain available and presentation is a no-op. A missing file or failed native
host request leaves the documentation intact and reports failure. Repair the
path or host and present the existing files again; do not restart docs writing
or implementation to recover presentation. A partially successful multi-file
request may leave panes open, so inspect existing host state before retrying;
cross-host retry deduplication is not an accepted guarantee. Desktop keeps an
active edit selected and adds other requested files as tabs. Native presentation
creates adjacent editor surfaces without replacing existing panes.

The remaining consequential uncertainty is whether the configured host really
delivers that interaction. Demo must challenge it with an unsaved active draft,
a saved correction consumed by implementation, and a presentation failure with
the original files still recoverable. Opening a different checkout, replacing a
draft, launching a fallback host, or repeating `write-docs` on iteration would
invalidate the result under already accepted requirements; none requires a new
product choice. Native terminal remote presentation and LOO-498 restoration
remain the explicit dependency boundaries above. No implementation, navigation
or delivery action is selected by this review.

- Gate: finish the remaining affected checks. Publication verification on the
  merged tree passed graph compilation/Task progression (13 tests), the two
  changed `world_recipes` scenarios, prompt goldens, Desktop action DTO round-trip,
  and README/index sync. The headless Desktop run passed 55 tests across TaskFiles,
  DesktopPaneControl and the action DTO fixture, with the optional native display
  diagnostic skipped. The focused Rust, formatting and Clippy results below remain
  applicable. No migration is added by LOO-728.
- Publication and configured-host demo/review remain the code Graph's later
  steps. No parent landing or extra approval boundary is authorized here.
  Demo must show the original files in the current host, preserve active edits,
  and show saved documentation guiding implementation without an added stop.
  The active Task Session is `fd5f2333c5f54054b40c4bb5e574a98a`; inventory confirms
  its checkout and availability, not the requested Rex placement. Placement
  confirmation remains with caller navigation/demo.

Reconciled 2026-11-04: GitHub still reports #1810 and #1814 open at the pinned
heads above. The available `origin/main` adds `ac8e088f5f` (#1809), native status
and reconnect corrections; it changes neither file presentation nor the accepted
docs-first sequence. This commit is not in this checkout, and its candidate-host
evidence cannot stand in for this Task's demo. Parent stack integration remains
separate. No new product decision is needed for this reconciliation.

Affected user docs: `README.md`, `docs/index.md`, `docs/orchestration.md`, and
`docs/surfaces.md`. The parent's `docs/workspaces.md` retains its explicit gaps.

Earlier integration checks and unexplained host-response timeouts are preserved
at `13e7d5dac8:scratch/write-docs-first.md`; isolated reruns passed there.

Check: `git diff --check` and `lf context` passed at realignment; reused the preceding current-tree passes for `cargo test -p loopflow --test integration work_open_tests:: -- --test-threads=1` (18 passed, 2 opt-in native cases ignored), `cargo test -p loopflow --lib lf::commands::desktop::tests` (13 passed; nonfatal macOS unwind-size linker warning), `cargo +stable fmt --all` and `cargo +stable clippy --all-targets -- -D warnings`. Gate owns broader affected suites; demo owns configured-host behavior.

Publication review, 2026-11-04: `lf sync` completed without conflicts against
LOO-498 at `da41d2031f`. PR #1817 was created as a draft at `6feff830f2` during
the checks; that commit contains the reviewed uncommitted integration work.
The code Graph publishes before its interactive demo, so publication does not
establish configured-host acceptance. Source #1810 and parent #1814 remain open
at their pinned revisions. The first README check used the root Python environment
and lacked Playwright; the documented website environment passed. Swift linking
reported nonfatal macOS deployment-target warnings. No configured host was opened
or store upgraded for these checks.
