# LOO-728 — integrate write-docs-first

Jack Heart requested integration on November 4, 2026, preserving source branch,
checkout, conversation and draft PR #1810 at
`5e6d3b1fc1ad23bb0fed51f4c0ad9815fed67648`. LOO-728 PR #1817 remains stacked
on LOO-498 PR #1814. Loopflow merge `0cb4421cb8` integrates published parent
`df3b5e810d3841a19bc5911c332fb42f23193d46`, including its coherent installer
and revised draft workspace schema. Integration does not authorize landing the
parent or waive its acceptance. [rex-workspace.md](rex-workspace.md) holds the
remaining dependency boundaries.

## Accepted experience and owners

Jack Heart's November 3 direction: explore with prototypes and diagrams, then
write the actual user docs before implementation. Jack chose user documentation,
not a press release. Sources: [user-docs-first](https://blog.larah.me/write-the-docs-first/)
and [working backward](https://maven.com/articles/start-with-the-press-release-amazon-method).

`pursue` begins at `write-docs`; both backward edges return to `implement`.
Review and editing remain available while work continues, with no extra approval
stop or scratch docs copy. Implementation reads saved docs initially and before
finishing, preserving corrections. Unsaved drafts belong to the editor; immediate
direction belongs in the Task conversation. No live synchronization is promised.

Jack Heart accepted **Desktop-only automatic presentation** on November 4 in
comment `50456e16-156c-424e-8fa1-67030496483f`, superseding automatic current-host
presentation. `write-docs` uses `lf open --host desktop --file PATH` for original
files. An already attached Desktop Files view renders Markdown/HTML and preserves
its draft/attention policy. Absent, unavailable or unmatched Desktop means report
the actual paths and continue, without launching an editor, browser, host or
conversation. There is no unresolved presentation-policy choice. A universal
rendered viewer is outside this Task; the current client is macOS and remote
relative assets remain unproved. [rendered-file-viewers.md](rendered-file-viewers.md)
retains the research.

Explicit, repeatable `lf open --file PATH` still presents original files through
`open::files::run` without starting a host or conversation:

- Desktop's `files(path)` action uses `TaskFilesStore::presentFile` and existing
  attention policy. Active edits stay selected; other files become tabs. Desktop
  retains its remote Files transport.
- Native hosts create adjacent editor surfaces through `task_tool::file_editor`.
  Rex reuses an existing file pane. Native editors require a local checkout.
- Transport's `control_state` supplies controlling-viewer placement through the
  existing Session summary. Passive viewers cannot redirect presentation;
  detached or unknown owners supply no destination. Direct native Sessions retain
  recorded placement. No new Session, Task state or schema is added.

No attached host means a successful no-op. Missing files and native failures
report failure without changing docs. Repair the path or host and present again;
do not rerun docs writing to recover. Inspect panes after partial multi-file
success before retrying; cross-host deduplication is not promised. Preserve
original files, drafts, panes and Session identity.

Affected user docs: `README.md`, `docs/index.md`, `docs/orchestration.md`, and
`docs/surfaces.md`. `docs/workspaces.md` remains the sole workspace guide.

## Delete — do not maintain

Completed cuts and surviving owners:

- Inline editor selection and local `viewer_argv(task tool)` launches → shared
  `task_tool::file_editor`; no installed-CLI override for presentation.
- Execution-process placement and boolean-only `transport::controlled` → current
  controlling-viewer placement from `control_state` and the Session summary.
- Repeated viewer reads, per-file Desktop setup and duplicate native traversal →
  one Desktop batch/pane and shared Rex traversal. Reports borrow paths;
  canonical paths compare identity, recorded checkout spelling drives execution.
- Automatic current-host workflow and terminal-demo acceptance → attached Desktop
  presentation. Explicit terminal opening and its regressions remain required.
- `desktop-only-direction.md`, `docs-first-dev-demo.md`, `workspace-demo.md` →
  accepted scope here, LOO-728 evidence in `docs-first-demo.md`, and parent evidence
  in `rex-workspace.md`. Their complete sources remain at `ef0ca500977a123076cf08f0e114b7fd59d36881:scratch/`.
- Duplicate embedded screenshots in `demo.html` → existing `docs-first-demo-assets/`.

No code predecessor or exclusive fixture remains to delete. The merged code
review found no further worthwhile reduction. Add no competing Files store,
viewer registry, host launcher or persisted review lifecycle.

## Remaining delivery and acceptance

Concept review, November 4: no design change and no open product choice. Jack
Heart's Desktop-only decision is represented in the usage docs, `write-docs`
instruction and existing `files(path)` action. The simpler model remains original
documentation plus the existing editor draft and Session; presentation creates
no review lifecycle, approval state or additional persistence. Keep the current
implementation direction.

Normal path: write `docs/usage.md`, present it through an attached Desktop,
save a correction, and let implementation reread that file. An active edit keeps
its selection and draft; another presented file becomes a tab. Failure path:
absent Desktop returns without opening anything; unmatched Files or failed
presentation reports an error, and the skill lists paths and continues. Recovery
retries presentation after fixing the destination or path, preserving the saved
docs and same conversation. A rendered view that replaces a draft, a retry that
rewrites docs, or a fallback that launches another host would invalidate this
contract. Configured Desktop proof below must exercise those counterexamples;
headless model checks do not settle them. Remote relative-asset parity remains
outside the accepted claim, not an unresolved choice for this integration.

November 4 reconciliation confirms PR #1814 remains open at the integrated
`df3b5e810d3841a19bc5911c332fb42f23193d46`; LOO-498 has active implementation
work, but no newer published parent is available. The earlier request to sync
and rebuild this parent is satisfied. PR #1817 remains open on the correct
LOO-498 base, but its published head is still `c136addfad33b65a4080f609f12da2c1a663b85c`.
The parent integration and accepted Desktop-only change at local
`ef0ca500977a123076cf08f0e114b7fd59d36881`, plus reconciled notes, remain unpublished.
Publication belongs to the caller's delivery step.

Parent delivery precedes supported shared-store promotion; the unmerged migration
cannot be bypassed or landed under LOO-728's approval. The coherent installer is
integrated but not installed. Preserve the primary writer, installed build, source
work, provider history and recovery bytes. The ignored `docs/.orchestration.md.swp`
remains local. No authority to restart the primary conversation follows here.

Demo/review owns configured Desktop proof: present original Markdown/HTML in an
already attached Files view, preserve an active draft on repeat, save and read the
correction from the same original, recover failed presentation, and continue the
same conversation. Outside attached Desktop, report paths and keep working without
launching anything. Headless checks establish neither mounted nor configured
acceptance. Do not repeat the superseded terminal automatic-presentation demo.

[docs-first-demo.md](docs-first-demo.md) preserves direct/native successes and
configured-route failure evidence. [questions.md](questions.md) retains integration
assumptions and the proposed correction for the authorized Wave-memory owner.

## Validation

The full pre-compression plan, checks and earlier archive references remain at
`ef0ca500977a123076cf08f0e114b7fd59d36881:scratch/write-docs-first.md`.

Check (realign, November 4): `git diff --check` passes and `lf context` fits both limits; code inspection agrees with the Desktop-only skill and explicit current-host opening. Reused implement results: `cargo build -p loopflow --bin lf`, migration-history check, fmt, `cargo clippy --all-targets -- -D warnings`, and `cargo test -p loopflow --test integration work_open_tests:: -- --test-threads=1` (20 passed, 2 native probes skipped; flaky mock-argv fixture removed), exact no-Desktop rerun and README/index sync passed. Unchanged graph/progression (13), two world scenarios, prompt goldens, action DTO and 55 headless Files/pane/DTO checks remain applicable. Gate owns broader affected suites; demo/review owns configured Desktop proof after compatible installation.
