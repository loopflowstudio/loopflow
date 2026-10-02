# Task history review · 2026-10-02

Jack Heart requested a reviewable LOO-369 change and a concrete Growth demo.
Publication is authorized; feature landing and demo approval are not.

Jack Heart's October 2 screenshot shows the six retained Tasks and no canceled
duplicates. Jack requested moving the underdesigned standalone completed checkbox
into the Tasks header. The revision puts compact history controls beside the
heading and count. Jack Heart subsequently approved the compact inline HTML
prototype and requested its Desktop implementation. The native revision uses
Completed / N Days / All Tasks, with zero meaning unbounded. Desktop visual
review and delivery remain the caller’s work.

## Open the prepared snapshot demo

```sh
/Users/jack/.lf-demo-loo369/launch.command
```

The branch app and matching CLI are built at
`~/Applications/Loopflow Dev.app`. The command opens this checkout using the
explicit private `LF_HOME=/Users/jack/.lf-demo-loo369`. It runs the packaged
native executable under `sandbox-exec`, with a clean environment, no network,
and no writes outside the disposable Home and system temporary directories.
Production Home and common credential directories are unreadable. Sandbox
restrictions apply to child processes, including Desktop's automatic CI watcher.
The private Growth placement is disabled for admission; no Tasks, Flows,
Sessions, landing operations, credentials, or cron registrations were imported.
Do not launch this snapshot through an unsandboxed `open` command.

Preparation used the documented launcher:

```sh
uv run python scripts/loopflow-dev.py install
```

This built SwiftPM, built the matching development/validation-only CLI, and
signed the separate Dev app. The production application and installed CLI were
not changed. An explicit private Home keeps the bundled source CLI from
forwarding to the older installed runtime; an installed-runtime upgrade is
**not** a prerequisite for this demo. The native process was launched successfully
and remained running with bundled CLI children. There is no rendering environment
in this preparation session, so this establishes launch, not visual acceptance.

### Captured data and observed checks

A read-only SQLite transaction captured Growth's Wave identity and existing
planning snapshot from the installed Home. Only that Wave and its one Project
and 13 planning items were imported into a freshly initialized private database.
The private Home has its own identity and a local disabled placement. Provider
states, issue identities, descriptions and timestamps were preserved; no runtime
or Session state was manufactured. Snapshot observation time is
`2026-10-02T09:56:54Z` (`1790935014`), not the demo launch time.

The **packaged source CLI under the launch sandbox** returned:

- LOO-318, LOO-315, LOO-314, LOO-313, LOO-310, LOO-308 and LOO-307:
  `state:canceled`, `completed:false`, `section:later`, canceled next-move reason.
- Six open Tasks: LOO-317, LOO-316, LOO-312, LOO-311, LOO-309 and LOO-306.
  All four requested retained targets are present. The expected default visible
  count is six; the full shared inventory still has thirteen.
- No successful completions in this snapshot. All completion dates remain
  unknown (`null`), because the captured provider data omitted them. No date
  was inferred from update or observation time.
- Zero local Tasks, FlowSessions, AgentSessions and landing operations.
  A sandbox write probe against the production checkout failed with
  `Operation not permitted`; network access was also denied.

Local artifacts in `~/.lf-demo-loo369/`: `growth-capture.json` preserves captured
planning, `sandbox-roadmap.json` records the matching CLI projection,
`snapshot.sb` defines isolation, `launch.command` launches, and `app.log` captures
native output. These disposable artifacts are local, not published fixtures.

### Jack Heart's review path

1. Select Growth and refresh its local projection. Completed starts unchecked.
   Confirm the seven duplicates are absent and all six open Tasks remain,
   including LOO-309, LOO-316, LOO-312 and LOO-306. The heading should say six.
2. Click Completed: 7 Days, then edit to 30, then 0 (All Tasks). Counts stay at six
   for this real captured data, because it contains no successful completions.
   Confirm the duplicates remain absent throughout.
3. Click All Tasks to edit 0 Days; enter -1. Confirm validation appears and
   the last applied range remains effective. Escape cancels an edit. Restore 30.

This is an offline snapshot demo. A provider-sync action cannot fetch fresh
facts under its sandbox. Recent/old completion rows and unknown-date successful
completions are covered by the existing synthetic headless acceptance below,
not by this Growth capture. Existing Task workspaces and native Sessions were
intentionally not imported: reopening and continuing a real retained Session
still needs a separate live review. Do not start work from this snapshot.
No visual approval, live refresh, Session continuity or Flow recovery is claimed.

## Inline revision evidence

`swift test --package-path swift --no-parallel --filter TaskHistoryFilterTests`
passed seven tests after compression (1.634s) and compiled the SwiftPM Desktop app. The production Wave binding still drives filtering and
counts. Native tests cover Enter, blur, Escape, invalid drafts, remembered ranges
and zero; headless bitmap captures compare number glyph bounds in display/edit
for 7, 30, 365 and Int.max, alongside checkbox/suffix frames and baselines.
Implementation review selected one actual native text surface in both modes. The custom
renderer is deleted: the same NSTextView displays and edits the number. Bitmap
comparisons capture the actual control through identical viewport origin, scale
and clipping, excluding selection paint; native glyph baseline coordinates are
checked separately. Both snapshots exclude selection paint and verify nonempty
glyph ink rather than an opaque viewport rectangle. Days activates editing too. Blur does not redirect focus;
Enter/Escape retain keyboard traversal and Tab/Shift-Tab apply and move focus. This is headless evidence, not a new native
visual approval. Long values scroll within the fixed numeric viewport. Windowed focus traversal and delivery belong to the caller. The revised gate
result below supersedes the earlier pending-gate note.

## Revised inline gate · 2026-10-02

The current revision passed 142 Rust tests: 123 affected library tests, 18 DTO
fixtures, and the terminal-Task admission regression. All 100 selected Swift tests
passed under `scripts/test_desktop.sh`, which denies WindowServer connections.
These include all seven Task history tests, native glyph bitmap comparisons,
production filter bindings/counts/actions, DTOs, registry reads and navigation.
No additional code repair was needed during this review.

Commands: `cargo nextest run -p loopflow --lib -E 'test(pm::linear::) | test(ops::pm::) | test(ops::linear_observe::) | test(store::sqlite::planning) | test(lf::commands::waves) | test(ops::task_flow)'`; `cargo test -p loopflow --test dto_fixtures`; `cargo test -p loopflow --lib task_preparation_rejects_unpublished_parent_before_allocating_child`; `scripts/test_desktop.sh --jobs 4 -Xswiftc -gnone --filter 'DTOFixtureTests|DesktopHeadlessTests|WaveDetailReadingTests|WorkspaceNavigationTests|RoadmapViewTests|TaskHistoryFilterTests|RegistryQueryTests|WaveLensTests'`.

Formatting, all-target Clippy, architecture and Swift boundary checks passed.
The bounded gate runner built the current CLI, SwiftPM app and configured signed
Xcode app/test runners. Xcode `build-for-testing` passed in 221 seconds; the four
selected gate suites completed in 572 seconds. Logs remain under
`.lf/tmp/gate/run-67566/`; the separate admission regression passed in 0.94 seconds.
The full matrix, including Linux-only deletion integration, remains with CI.

Review confirmed that absent completion dates can be enriched without weakening
same-revision conflicts, and that removed settled checkouts cannot bypass history
filtering. Native appearance/focus, live Growth refresh and real retained Session
continuation still require the authored demo. Compilation does not establish them.

For later Product measurement, extend `hierarchy_interaction_ms` with range
application through visible rows and count at the existing small/large populations.
This proposes a measurement; these headless checks establish correctness, not
rendering latency or a sustained-use KR.

## Earlier headless evidence

The shared synthetic fixture crosses Linear decoding, PM storage and Rust Task
projection, then RegistryQuery and the production Swift views. Every history
setting asserts all seven exclusions, unresolved canceled work, row counts and
row-opening actions. The production Task Start button rejects canceled/completed
inventory and remains enabled for the retained open target. Synthetic Session
membership and the affected navigation suite pass; native provider continuation
remains part of the walkthrough.

Rust: 145 tests passed across Linear, PM operations/observation, planning storage,
Wave projection, Task Flow, decision schema and DTO fixtures; two subprocess
entry points intentionally ignored. Swift: 68 affected tests passed, then all
five Task history tests passed after adding the Start-button assertions.
Formatting, all-target Clippy, architecture and Swift boundary checks passed.
The configured gate runner's `loopflow` suite passed signed Xcode
`build-for-testing` (77 seconds). SwiftPM compiled the app with the view tests.
The full matrix remains CI's responsibility; checkpoint scratch defers hosted
CI under repository policy.

Local logs: `.lf/tmp/task-history-rust.log`, `task-history-swift.log`,
`task-history-controls.log`, and `.lf/tmp/gate/run-87473/loopflow/xcodebuild.log`.
The first three filenames are under `.lf/tmp/`. Logs are local artifacts, not
published assets.

## Review findings and boundaries

The simulated code review preserved one terminal-state classification and one
history filter, kept unknown dates distinct from recent completion, and checked
that removed historical checkouts do not resurrect settled work. Gate closed
the intermediate-window and production action test gaps. No second archive,
timestamp backfill, lifecycle mutation or display-owned execution authority was
introduced.

The structured Flow decision schema repair is identical to active base
`2064555c4` after integration; it no longer adds a feature-branch overlap.
Historical failed-occurrence evidence does not establish current Flow state,
provider acceptance or recovery, and does not authorize navigation.
