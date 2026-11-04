# LOO-728 — integrate write-docs-first

## Current delivery direction — independent main

Jack Heart's November 4 [independent-main direction](main-delivery-direction.md)
supersedes the stacked delivery requirements below. Preserve the accepted
Desktop-only automatic presentation and saved-doc workflow. Extract those changes
onto fresh main using its existing Desktop Files support; leave parent-dependent
terminal/Rex additions, migration, installer and Wave-memory changes in preserved
source/stack history. Preserve Task identity, PR #1817, source work and LOO-498.
Managed Task placement, PR base and Git must agree before independent delivery.

Demo decision pass 1 in Session `session_edb0e699d5c14fcc9223247d196d01d6`
returns **iterate**: the Graph resumed at demo while HEAD remains the stacked
`81cf57afe9` candidate. Checkpoint current edits, perform the extraction and
managed detachment, reconcile this plan to the result, and validate against main.
See [current demo evidence](docs-first-demo.md#current-decision-pass-1--independent-main-work-remains).
Earlier checks and advance decisions below describe the prior stacked candidate;
they do not prove the independent-main outcome. Attached Desktop proof remains
limited as recorded. No new product judgment is required.

## Prior stacked implementation and evidence

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
LOO-498 base. Gate's fresh GitHub read confirms its published head is
`81cf57afe949d3f4b8f639b01c786073d16d5b5f`, including the parent integration,
accepted Desktop-only change and reconciled notes. The subsequent headless demo
decision and gate evidence remain local; publication belongs to delivery.

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

Gate plan, November 4: compare against the integrated LOO-498 parent `df3b5e810d`,
including the existing unstaged demo notes. `scripts/test.py --base df3b5e810d
--list` selects architecture, Rust, website and headless Swift. Rust coverage:
`open`, `desktop`, `task_tool`, `human_session`, `transport` and `work_open_tests`
for file destinations, control placement and unchanged explicit opening;
`graph::compiled`, `task_graph`, `skills::catalog`, prompt goldens and the two
changed world scenarios for docs-first entry and loop preservation; documented
commands and DTO fixtures for the public command/action boundary. The runner
also builds the candidate CLI, checks formatting and runs all-target Clippy.
Website checks use the candidate CLI; Swift builds Desktop and exercises its
headless model/view suite and platform boundaries. Native probes remain skipped;
Linux transport/reconnect and disposable installation proofs remain with CI.
No installation, parent operation or display-host launch belongs to this gate.
No new performance mechanism is introduced: the production signal remains time
from the existing Files action receipt to document readiness; attached-view
render latency is unmeasured here.

Gate repair: the integrated parent's `host_workspaces`, `host_work_occurrences`,
`wave_task_layouts` and Rex process edge were absent from the checked architecture
inventory. `docs/architecture-reference.md` now records their existing owners,
without changing the schema or native ownership contract. Check:
`uv run python scripts/check_architecture.py` — passed after the map correction
(22 APIs, 81 tables, 35 subprocess edges; zero unexplained drift).

The first Rust pass ran 252 tests: 251 passed; the Desktop-placement fixture
failed because `Fixture::command_from` cleared cmux but inherited the invoking
Rex context. Its invalid-Desktop case therefore recorded the real ambient Rex
placement. The transport fixture now clears Rex, Herdr and tmux context alongside
LF/cmux. Rerun only `binary(transport_tests)` under the same ambient host, followed
by formatting and all-target Clippy; retain other Rust and website passes.

Gate check results (November 4; commands unset inherited `LF_*` and close stdin):

- `scripts/test.py --base df3b5e810d --reuse-passing --rust-filter <planned modules above>` — initial run retained at `.lf/tmp/gate/run-10562`; its aggregate failed and is not relabeled green. The log contains the exact expanded filter and commands.
- `cargo nextest run -p loopflow --no-fail-fast --test transport_tests` through `scripts/test_network.py` with the prepared candidate — 55/55 passed after fixture isolation; the other 197 selected Rust tests retain their pass. The two opt-in native work-opening probes were skipped.
- `cd website && uv run python dev.py test` with candidate `LF_BIN` — 72 passed, including rendered docs, accessibility and README/index sync; 34.2s phase wall time.
- `scripts/test_desktop.sh --jobs 4 -Xswiftc -gnone` — app/test build passed in 55.7s; the full suite exited 1 after starting `ThemeTests.defaultConfigurationLoadsAfterInitialAppearance`, without a reported assertion or timeout. Its cause remains unresolved; full-suite completion is deferred to CI, not passed.
- `swift test --skip-build --disable-sandbox --package-path swift --no-parallel --filter ThemeTests/defaultConfigurationLoadsAfterInitialAppearance` through the headless/network/time-limit wrappers — 1 passed; CoreVideo explicitly reported no display link and used timer rendering. This isolated pass does not explain or erase the full-suite exit.
- Same headless command with `--filter 'TaskFilesTests|DesktopPaneControlTests|DTOFixtureTests'` — 105 passed in 11.7s, including original-doc presentation, draft retention, rendered Markdown/HTML selection, saves and action DTOs; optional display diagnostics skipped.
- `uv run python scripts/check_swift_multiplatform_boundaries.py` — passed.
- `cargo fmt --all` and `cargo clippy --all-targets --jobs 4 -- -D warnings` — passed after the fixture repair; Clippy completed in 1m40s. `git diff --check` also passed.
- `uv run python scripts/resource_envelope.py --json` — passed on remeasurement, 210.0 GiB free, no measurement issues. The initial resource failure was the shared 11.5 GiB uv cache inventory exceeding its 15s scan deadline, not a product-test timeout; transient scan latency's cause is unproven. No cleanup or budget change was needed for the remeasurement.

Follow-up logs and the resource receipt are retained beside the original run as
`recheck-transport.log`, `recheck-swift-theme.log`, `recheck-swift-files.log` and
`recheck-resources.json`; `recheck-clippy.log` records the final lint. The affected headless Desktop checks establish the
automated boundary; they neither establish attached Desktop acceptance nor
clear the unexplained broader Swift run. Keep that limit in delivery context.

Demo decision pass 2, November 4: the agent, explicitly standing in for Jack
Heart, advanced the headless review after real-Task candidate commands proved
the accepted no-Desktop fallback, missing-path failure and retry without changing
either original document. The configured Desktop socket was unavailable before
and after. This settles the decision at this step; it does not establish attached
rendering, draft preservation or UI save/readback, nor change parent delivery or
installation authority. Exact observations and rationale are in
[docs-first-demo.md](docs-first-demo.md#earlier-decision-pass-2--headless-desktop-fallback).

Earlier demo decision pass 1, November 4, Session `session_3cb2a0ef951d48a3a1cabc953b97685e`:
advance under delegated judgment. The gate-built candidate also demonstrated
Session-inferred original paths, absent-Desktop fallback and missing-path retry
without a Task override or changed document bytes. Existing affected check logs
were inspected, not rerun. Attached UI proof and the broader Swift suite limit
remain open evidence limits. See [current demo evidence](docs-first-demo.md#current-decision-pass-1--configured-session-fallback).

Current demo decision pass 1, November 4, Session
`session_74d8335b541542b28c4f73e4d0ed5764`: advance under delegated judgment.
The same configured Session command, missing-path failure and retry reproduced
the accepted absent-Desktop behavior with unchanged original document hashes.
No new implementation defect or product choice was found. Retained affected
checks remain applicable; attached UI proof and the broader Swift exit remain
unresolved evidence limits. No installation or parent operation was performed.

The full pre-compression plan, checks and earlier archive references remain at
`ef0ca500977a123076cf08f0e114b7fd59d36881:scratch/write-docs-first.md`.

Gate follow-up, November 4, Session `session_cb859e7fee38428e82a5b669d7ce5417`:
reviewed the complete parent-relative code and documentation diff plus the
existing unstaged fixture and architecture repairs. No further in-scope defect
was found. Original-file identity, controlling-viewer ownership, Desktop-only
automatic presentation and implementation loop targets agree with the accepted
experience. No production or test source changed in this pass, so the affected
Rust, website, Files/pane/DTO and lint results above remain applicable.

- `scripts/test.py --base df3b5e810d --list` — same affected suite inventory; no full-matrix execution.
- Headless/network/time-limit wrappers around `swift test --skip-build --disable-sandbox --package-path swift --no-parallel --filter ThemeTests` — 21 passed in 0.418s; log and event stream: `.lf/tmp/gate/run-10562/recheck-swift-theme-suite.{log,jsonl}`.
- `git diff --check` — passed.

The theme-suite diagnostic narrows the earlier unexplained exit: it does not
reproduce within that suite, including the previously unfinished test. The only
nearby Loopflow crash report was a separate 14:14 Sparkle packaging fixture,
before the Swift run's 14:21 start; it does not explain the failure. No timeout
or assertion was recorded in the failing log. Broader Swift completion remains
deferred to capable CI, with the original failed run preserved; neither focused
pass clears it. No new display, host, installation or parent operation was run.

Check (realign, November 4): `git diff --check` passes and `lf context` fits both limits; code inspection agrees with the Desktop-only skill and explicit current-host opening. Reused implement results: `cargo build -p loopflow --bin lf`, migration-history check, fmt, `cargo clippy --all-targets -- -D warnings`, and `cargo test -p loopflow --test integration work_open_tests:: -- --test-threads=1` (20 passed, 2 native probes skipped; flaky mock-argv fixture removed), exact no-Desktop rerun and README/index sync passed. Unchanged graph/progression (13), two world scenarios, prompt goldens, action DTO and 55 headless Files/pane/DTO checks remain applicable. Gate owns broader affected suites; demo/review owns configured Desktop proof after compatible installation.
