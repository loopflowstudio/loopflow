# LOO-711 native update demo · 2026-11-02

Outcome: partial configured-provider proof, incomplete Desktop demo. Available for Jack Heart's review; no acceptance recorded. LOO-711 remains at interactive `demo`, with no PR and no graph continuation.

## Experience and setup

Jack Heart invoked `$demo` for worker posts into the existing Task Session and the separate compact run page. Current design: [loo-711-task-sessions-tell.md](loo-711-task-sessions-tell.md). Review page: [demo.html](demo.html); machine-readable evidence: [loo-711-demo-evidence.json](loo-711-demo-evidence.json).

Code: `eb08f96a74f3bc9d6956b8b9a743d1c1b29a2d29`. Branch `cargo build --locked -p loopflow --bin lf`, FFI library build and `swift build --jobs 4 -Xswiftc -gnone --product LoopflowMac` succeeded. These are build evidence, not broader gate results.

A separate local bundle at `/Users/jack/Applications/Loopflow LOO-711 Demo.app` uses private `LF_HOME=/tmp/loo711-demo/home`. The disposable Git repository is `/tmp/loo711-demo/worker-demo`; its Task is `06bf2e` (`task_06bf2e26aea44545a32cad485686ed70`). Its custom task-session instruction says to observe only and never advance work. This exercises the production native transport with the real configured Codex provider, but does not prove the default operator skill's full behavior. No production Task, provider credential or shared-store migration was changed.

Native Session: `session_f1478d6846224f4dbc3384f9c37147dd`; provider thread `01a19f43-3b41-72d3-bfff-88027bca6314`; Codex 0.166.0, GPT-6-Astra high. Terminal capture used a real native TUI under a PTY, not a mocked provider. Jack did not personally perform or confirm these checks.

## Observed by the assistant

- Initial response: “Ready for the demo.” The Session then became available. The exact unsent draft was `Draft: keep this sentence while the worker changes.` No Enter was sent.
- A harmless Graph ran `lf help`, then stopped before interactive `demo`. Its progress observation was dispatched and accepted one second after completion. A native turn started in the same thread without a submitted user message.
- The first report completed in **267,664 ms**: “Task 06bf2e’s demo run and help step succeeded; it’s available for review.” Earlier live observations at 3m39s showed Working with no answer; the final evidence supersedes the provisional diagnosis of a permanently stalled report. The cause of this first delay is unknown; subsequent reports took about 3–4 seconds.
- An independent run completed while that turn was busy. Its final observation was delivered after the turn completed (not a burst of every intermediate state). That run was named `failure` but actually succeeded: `task wait --timeout` returns zero. A second setup attempt using missing Task status also returned zero. These are fixture setup mistakes, not failure-path proof.
- The final controlled failure used `lf help demo-intentionally-missing`, exit status 2. The report correctly said: “Task 06bf2e remains available for review at demo; the latest run and its `help demo-intentionally-missing` step failed.” This report took 3,516 ms. Task position remained `demo`; no work was restarted.
- Native redraw after all reports still showed the exact unsent draft. Session and provider-thread identities remained the same. This establishes draft retention through these observations, not across provider transfer or restart.
- **Visible noise:** each automatic observation appears as a full user-style `<lf:task-update>` block, including Graph/Process IDs and machine-facing instructions, before the short assistant reply. This may conflict with Jack's accepted low-noise direction. No usability acceptance is inferred.

## Desktop demo failure

The initially selected installed Dev app displayed a shared-store schema mismatch for unmerged migration `wave_removal`; it was not the LOO-711 build. Its `desktop state` request also timed out. No migration or replacement installation was attempted.

The isolated branch app launched, but restored `/tmp/loo-500-synced-demo/repo/`. `lf open` for fixture Task 06bf2e returned `opening` while the visible window remained that unrelated empty repository. Direct `--repo` launch also retained it. The Open Repo picker displayed the correct fixture folder with its Open Repo button disabled. A first bundle in `/tmp` was not resolved by Launch Services; placing the isolated bundle in Applications resolved registration but not the visible navigation failure. Root cause and attribution to this diff are unproven.

Actual before/after navigation snapshots: `/tmp/loopflow-20261102-183902.png` and `/tmp/loopflow-20261102-184100.png`, embedded in the page. Both show the failed entry state. They are **not** before/after implementation screenshots and establish no run-page acceptance. Native run expansion, exact failed-Process navigation and Files interaction remain unshown.

## Artifact validation

The self-contained page passed HTML parsing, unique-ID, embedded-asset and JavaScript syntax checks. Its actual native screenshots were visually inspected. Browser security policy rejected opening a local file URL, and no workaround was attempted; rendered HTML interaction and narrow/dark appearance remain unverified. Jack can open the saved file directly.

## Feedback and design

Jack Heart has not yet supplied feedback on this demo. Existing decisions remain worker posts plus screen 2's compact run page, combined position/readiness and Files-owned diff. No new design change is agreed.

Proposed next direction: expose a concise progress post without displaying the transport envelope; retain exact evidence in history. Investigate the first report's four-minute delay and establish a working native Task opening before repeating the run-page demo. These are proposals and investigations, not an accepted redesign or proven root cause.

## Remaining proof and next action

Reconnect without duplicate/uncertain replay, all other configured providers (Claude, OpenCode, Pi), mounted run-page interaction, cadence/noise judgment and broader gate remain. The native draft specimen and isolated app are left available; no Session was completed or closed. No worker restart, publication or merge followed this review.

Recommended next action: review `scratch/demo.html`, then use the existing Task for a bounded repair/demo pass chosen by Jack Heart. Preserve the first-delay and opening failures. Do not call the overall demo successful from the native delivery evidence alone.
