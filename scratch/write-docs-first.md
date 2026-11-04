# LOO-728 — integrate write-docs-first

Jack Heart requested integration on November 4, 2026, preserving the source
branch, checkout, conversation and draft PR #1810 at
`5e6d3b1fc1ad23bb0fed51f4c0ad9815fed67648`. LOO-728 remains stacked on LOO-498
(PR #1814); its own PR #1817 is open. Merge
`c136addfad33b65a4080f609f12da2c1a663b85c` includes parent
`b8aac2ef139ff34213685a10137572e1498f5e4f` and upstream `ac8e088f5f` (#1809).
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

`lf open --file PATH`, repeatable, presents the original files through
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

No code predecessor or exclusive fixture remains to delete. The current diff
review found no further worthwhile code reduction. Repeated demo instructions
and diagnoses now live once in `docs-first-demo.md`; superseded iteration
narratives are retained in Git. Add no competing Files store, viewer registry,
host launcher or persisted review lifecycle.

## Remaining delivery and acceptance

November 4 reconciliation found parent PR #1814 still OPEN, now at
`df3b5e810d3841a19bc5911c332fb42f23193d46`. This checkout still contains
`b8aac2ef13`. The newer parent includes LOO-723's coherent installer and revises
the draft `rex_workspaces` schema, native creation receipts, follower behavior
and workspace documentation. Synchronizing that parent through Loopflow and
resolving the overlapping file-presentation code/docs remains substantial
integration work. The finished parent migration and rebuilt catalog must agree
before promotion; the existing candidate's preflight describes the older schema.

The configured route still fails before opening an editor. The previous
implementation pass repaired the candidate's migration order by restoring full
Git history and rebuilding. Promotion now fails solely on LOO-498's unmerged
`rex_workspaces` migration; this Task cannot land that parent or bypass the
shared-store boundary. Exact commands, owner reply and historical failures:
[docs-first-demo.md](docs-first-demo.md).

After parent delivery, the existing installation path must deliver a coherent
build containing this stack and transition the same Session to a compatible
owner/viewer. LOO-723's `dfd454cfc783b5ff3014d5a398ccf1627b427d11`
(#1815, present in local `origin/main`) promotes matching Desktop/CLI artifacts
while retaining running helpers and Graphs. Its `release/README.md` distinguishes
installed bytes from owner processes; eligible owners use existing idle rollover.
That source is now in the newer parent, but has not been integrated or installed
in LOO-728. Newer main alone does
not contain this stack or prove viewer placement. LOO-723 also records Jack
Heart's report of lost input/crashes after reopening, with cause unresolved and
hooks/reconnect owned by LOO-724. Require a completing conversation turn, not
just helper survival, to establish continuity.

Demo/review must use the configured agent route with provenance intact:

1. Open the original docs beside the retained conversation.
2. Repeat presentation while an unsaved edit exists; preserve its editor/draft.
3. Save and show implementation reading that correction from the original file.
4. Recover a failed presentation using those files; complete a conversation turn.

Preserve Session identity, provider history, active work and drafts through the
transition. Another checkout, removed agent provenance, startup-placement
fallback, passive-viewer redirection or rerunning `write-docs` invalidates proof.
Jack Heart reserved interactive Rex demo for the primary conversation. Existing
ship authorization is unchanged. Do not repeat an unchanged implementation/demo
loop before the delivery prerequisite is met.

## Direction and dependency boundaries

Delegated judgment retained Jack Heart's accepted current-host design. His
later Desktop-only automatic-presentation proposal remains unaccepted and does
not block this implementation. [rendered-file-viewers.md](rendered-file-viewers.md)
retains the research; `concept-review.html` contains proposed wording. Terminal
editing is not rendered viewing. Desktop's macOS client and unproved remote
relative assets do not establish universal rendering. If the proposal is later
accepted, change automatic workflow presentation and its docs while preserving
explicit `lf open --file` and existing Files/transport owners; it authorizes no
new viewer server today.

Jack Heart's November 4 parent decision assigns native restoration and shells
to Rex: LF launches templates, preserves retained destinations and never closes
Rex shells. Retained Task Session terminals prevent cleanup regardless of focus;
confirmed final removal releases that hold, while unknown inventory does not.
This supersedes the inherited full-restoration and Rex-settlement requirements,
without narrowing docs presentation or accepting the Desktop-only proposal.
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

Applicable merged-tree evidence: graph compilation/Task progression (13 tests),
two changed world scenarios, prompt goldens, action DTO and README/index sync
passed. Headless Desktop passed 55 TaskFiles/pane/DTO tests; optional display
diagnostics were skipped. The isolated real-Rex candidate probe passed opening,
draft preservation, saved-file readback and missing-file recovery. Jack Heart's
“ok it opened” confirms direct development CLI opening. Neither proves configured
agent-route acceptance; [docs-first-dev-demo.md](docs-first-dev-demo.md) retains
that demo, screenshots and its limits.

Check (realign, November 4): `git diff --check` and `lf context` pass;
production code is unchanged, so prior focused results apply. Gate owns affected
suites and Clippy after a code change; demo/review owns configured proof after
parent delivery, compatible installation and same-Session owner transition.

Historical plan/checks: `13e7d5dac8:scratch/write-docs-first.md`; publication:
`c136addfad33b65a4080f609f12da2c1a663b85c:scratch/write-docs-first.md`.
Pre-compression working plan, including prior unpublished reconciliation, is
preserved as Git blob `84005cbc45e3a58810ae609439fd87007c550fe4` (read with
`git show <blob>`). Its earlier archive is `7234a2e609b71e189e0020f19bcc9817441c89b5`.
