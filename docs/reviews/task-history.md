# Task history review · 2026-10-02

Jack Heart requested a reviewable LOO-369 change and a concrete Growth demo.
Publication is authorized; feature landing and demo approval are not.

## View the changed Desktop

From this checkout, use the configured development launcher:

```sh
cd /Users/jack/src/loopflow.keep-current-tasks-visible-and
uv run python scripts/loopflow-dev.py run-debug
```

This builds the SwiftPM app, packages it at `~/Applications/Loopflow Dev.app`,
and opens this repository. It uses the native Ghostty dependency and keeps the
Dev app's preferences separate. No launch or installation was performed during
this gate. The signed Xcode compile artifact is also available locally at
`swift/.build/xcode-derived-data/Build/Products/Debug/Loopflow.app`; compilation
is not a native Session demo.

**Live demo prerequisite:** the installed runtime must include this PR's PM
projection. The configured Dev app bundles a development CLI that forwards
ordinary commands to the installed runtime. Building the app does not update
that runtime. The October 2 configured `lf roadmap --wave growth --json` read
still omits `state` and `completed_at`; LOO-318 remains `completed:false` and
`section:available`. Launching the new app against that runtime can show the new
controls but cannot demonstrate correct cancellation or recent history.
Deploying a compatible runtime requires separate authorization; this gate did
not install source, recover the blocked Flow, or modify provider state.

After the compatible runtime is deployed, check `lf roadmap --wave growth --json`:
Task summaries must carry `state` and `completed_at`, and LOO-318 must retain its
canceled state. Then launch the Dev app using the command above.

1. Select Growth and refresh. Show completed starts off. LOO-318, LOO-315,
   LOO-314, LOO-313, LOO-310, LOO-308 and LOO-307 should be absent. LOO-309,
   LOO-316, LOO-312 and LOO-306 should remain reachable.
2. Enable Show completed. It starts at 7 days. Change the value to 30, then
   select All time. Successful completions enter according to actual completion
   dates; the count matches the rows. All time also admits unknown completion
   dates with a date-unavailable label. The seven duplicates stay absent.
3. Enter 0 with All time off. A validation message appears and the last valid
   window stays in effect. Restore 30.
4. Open a retained Task workspace and its existing Session, visit Growth,
   change the history setting, and return. Confirm native Session access and
   input are retained. Do not start or settle a Flow to demonstrate the filter.

These are proposed review steps, not observed live results. No screenshot,
mock view, provider simulation or old installed UI supplies demo approval.

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
