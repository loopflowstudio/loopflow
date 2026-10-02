# Task history review · 2026-10-02

Jack Heart requested a reviewable LOO-369 change and a concrete Growth demo.
Publication is authorized; feature landing and demo approval are not.

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

1. Select Growth and refresh its local projection. Show completed starts off.
   Confirm the seven duplicates are absent and all six open Tasks remain,
   including LOO-309, LOO-316, LOO-312 and LOO-306. The heading should say six.
2. Enable Show completed: 7 days, then 30, then All time. Counts stay at six
   for this real captured data, because it contains no successful completions.
   Confirm the duplicates remain absent throughout.
3. With All time off, enter 0. Confirm validation appears and the last valid
   filter remains effective. Restore 30.

This is an offline snapshot demo. A provider-sync action cannot fetch fresh
facts under its sandbox. Recent/old completion rows and unknown-date successful
completions are covered by the existing synthetic headless acceptance below,
not by this Growth capture. Existing Task workspaces and native Sessions were
intentionally not imported: reopening and continuing a real retained Session
still needs a separate live review. Do not start work from this snapshot.
No visual approval, live refresh, Session continuity or Flow recovery is claimed.

## Headless evidence

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

The independently published [Fix structured Flow decisions rejected by Codex ·
PR #1401](https://github.com/loopflowstudio/loopflow/pull/1401) remains open at
this review. Its existing repair is preserved in a separate local checkpoint;
this feature branch currently includes it until upstream integration removes
that overlap. It does not authorize retrying the saved decision occurrence.
