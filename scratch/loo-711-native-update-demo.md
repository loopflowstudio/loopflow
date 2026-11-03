# LOO-711 native update demo · 2026-11-03

Outcome: quiet automatic reports were observed in all four configured native harnesses: Codex, Claude, OpenCode and Pi. Each checked update preserved its unsent draft. These are assistant-observed native demonstrations, not Jack Heart's usability acceptance. LOO-711 remains at interactive `demo`; PR [#1806](https://github.com/loopflowstudio/loopflow/pull/1806) is open and landing is not armed. Jack Heart subsequently authorized shipping on November 3 after the outstanding issues were explained. The shipping process owns publication, landing and final verification. 

## Jack Heart’s review · 2026-11-03

Jack said, “ok, this looks pretty good”, then asked for known outstanding issues. This recorded positive presentation feedback; Jack subsequently said “lets ship it”, authorizing publication and landing while leaving untested behavior explicitly unverified. Remaining discussion: busy-period bursts, reconnect uncertainty, the unexplained initial 4m28s turn, broader gate and exact failed-Process navigation. The quiet-message changes are still local.

## Quiet reports across all harnesses · 2026-11-03

Jack Heart requested hidden or less noisy updates, preferably from an agent rather than the user, and explicitly required all harnesses. The current enum contains Codex, Claude, OpenCode and Pi; all four were exercised through their real configured native clients in the isolated Desktop app. The earlier raw blocks remain in historical conversations; this change alters new reports only.

| Native client | Incoming update presentation | Observed proof |
| --- | --- | --- |
| Codex 0.166.0 | Retained named tool output, hidden from the normal transcript; only the assistant reply appears | `quiet-update` succeeded; same provider thread, exact unsent draft retained. Screenshot `102055`. |
| Claude 2.1.317 | Collapsed peer message from `Loopflow worker progress`, followed by the assistant reply | Repeated success and controlled failure reports; final failure `73055e97` arrived with the exact draft intact. Screenshot `104146`. |
| OpenCode 2.2.1 | Synthetic context, rendered as one quiet “Loopflow worker progress” notice | Initial catch-up reported successes and the controlled failure; a second report handled new success `365e2c07` and preserved the draft. Screenshot `103943`. |
| Pi 1.3.0 | Hidden custom follow-up message; only the assistant reply appears | Repeated successes and failure `38c69fc1` reported automatically; exact draft retained. Screenshot `103751`. |

Screenshots are `/tmp/loopflow-20261103-<suffix>.png`, embedded in [demo.html](demo.html). Captures, receipt summaries, provider identities and replies are retained under `quiet_all_harnesses` in [loo-711-demo-evidence.json](loo-711-demo-evidence.json). The fixtures use configured models (Codex GPT-6-Astra, Claude Opus, OpenCode Claude Haiku, Pi Gemini through OpenCode); model selection was not changed for these reports.

The first OpenCode attempt never posted because its display availability was false despite its idle native client. Claude had the same condition. Delivery now uses OpenCode's owned stream busy state and Claude/Pi's native next/follow-up queues, rather than waiting for a stale availability projection. OpenCode synthetic inputs also participate in request settlement, so one update does not leave delivery permanently busy. The initial Claude peer payload rendered a verbose fallback; the native cross-session envelope produces the collapsed attributed message. Its earlier escaped-tag preview was then replaced with the actual progress text. These failures are retained here rather than counted as successful attempts.

Busy behavior is not identical: Codex/OpenCode coalesce while their stream is busy; Claude/Pi admit settled observations to their provider queues. A full native burst/long-running-turn matrix remains unmeasured. No guarantee of latest-only coalescing across all providers is claimed. Receipt tests cover deduplication, uncertain reconnect and provider fencing; native restart retained Claude/OpenCode conversation history, but comprehensive reconnect and takeover behavior is not proved. Draft preservation here applies to automatic updates, not client replacement or Session switching. Demo binary refreshes required resuming clients; no unsent real-user input was present or submitted.

Checks: `cargo test -p loopflow --lib task_update` (11 passed); Pi extension test (passed); branch CLI build (passed); `cargo clippy --all-targets -- -D warnings` (passed); formatting and diff checks (passed). The existing test-linker unwind warning remains. These are focused checks, not a full gate or acceptance of the run-page interactions.

Current disposition: **available for demo review** in the isolated app, with Claude selected. Fresh production status reports `feature` at `demo`; both associated Graphs are completed and `lf ps` shows no live Graph in this checkout. Fixture Task 06bf2e also remains at `demo`. No production Graph was launched, review advanced, PR published, or landing armed in this pass. Recommended next action: review the four native presentations and decide whether Claude's collapsed excerpt and OpenCode's notice are quiet enough. Exact failed-Process navigation and the broader gate remain separate proof.

## Color retry and in-Session demonstration · 2026-11-03, 10:09–10:11 PST

Jack Heart requested: “ok can you retry the color with demo and demonstrate the in-session stuff”. The assistant confirmed the composer was empty and the provider idle, exited the disposable native client with `/quit`, then clicked Desktop Resume. Session `session_f1478d6846224f4dbc3384f9c37147dd` and provider thread `01a19f43-3b41-72d3-bfff-88027bca6314` remained the same. This was a client restart, not a conversation reset. Colored command text, model name and working-directory text became visibly present.

The assistant entered `Unsent demo draft — keep this while the worker reports.` without submitting it. Independent fixture Graph `color-demo` ran only `lf help` and completed. An automatic report arrived in the existing native conversation and its reply completed in 4,704 ms. Independent fixture Graph `failure` then ran the intentionally missing help command, whose step exited 2. Its automatic failure reply completed in 3,161 ms. These are model-turn durations, not end-to-end transport measurements. No user message was submitted to trigger either report. Both screenshot and terminal capture show the exact unsent draft retained after both turns. The Task stayed at interactive demo.

Observed native before/success/failure screenshots: `/tmp/loopflow-20261103-101011.png`, `/tmp/loopflow-20261103-101039.png`, `/tmp/loopflow-20261103-101102.png`, embedded in `scratch/demo.html`. New receipts and terminal captures are retained in `scratch/loo-711-demo-evidence.json` under `native_retry`. The same raw transport envelopes remain visible; quiet presentation is not accepted. No new approval was inferred. The app remains open for Jack's review.

Recommended next action: Jack reviews the current native conversation, reporting cadence and noise. No further demonstration setup is required for this Codex case. Other providers, exact Process navigation and comprehensive reconnect behavior remain unproved.

## Earlier in-app retry · 2026-11-03

Jack Heart clarified: “i want to demo it in the app.” The terminal evidence and HTML account do not satisfy that interaction request. His question about the 4m28s result was answered as model-turn duration, not transport latency; no timing cause or acceptance was inferred.

The existing preview restored after choosing macOS “Don’t Reopen” for its crashed windows. Its fixture Task became visible, but opening a pane triggered another app termination. A debugger reproduced EXC_BAD_ACCESS while a cooperative Swift executor called `lf_write`; the failing instruction was a stack probe, with `LoopflowLibrary.call` in the stack. The earlier snapshot-only navigation diagnosis is incomplete: native layout persistence itself can exceed the debug build's cooperative stack.

A local repair moves synchronous one-shot FFI calls off `Task.detached` onto native threads with an 8 MiB stack. The existing watch owner is unchanged. A focused regression invokes the real library from an async caller and verifies it writes a graph customization. The rebuilt native app and focused regression passed: `scripts/test_desktop.sh --jobs 4 -Xswiftc -gnone --filter WorkObservationTransportTests/writeFromAsyncCaller` (1 test). The repaired app survives Task focus and graph-log opening through the Desktop API; its process remained alive for several minutes. The API reports the correct fixture Task, existing Session (elsewhere, with Move here available), and selected Graph processes tab. This is application-state evidence, not proof of visible rendering. UI control then returned `cgWindowNotFound` for Loopflow, Finder and Chrome. Jack was asked whether the desktop is unlocked; the answer is pending. No new native screenshot or completed in-app interaction is claimed. No Graph was restarted and no new design choice was attributed to Jack Heart.

## Native screenshot and color finding · 2026-11-03

Jack Heart supplied a screenshot of the fixture Task open in the app, with the Codex conversation and compact Graph processes pane visible, and asked “did we lose color”. This establishes native mounting, superseding the earlier inability to observe the window. It does not establish acceptance or draft preservation: the screenshot shows a submitted draft specimen and a Working turn.

Process inspection confirmed the original demo Codex client (PID 49248) inherited `NO_COLOR=1` and `TERM=dumb` from the assistant's terminal launch. The newly attached Desktop launcher (PID 92273) has `TERM=xterm-256color`, `COLORTERM=truecolor`, and no `NO_COLOR`. Attaching preserved the original native process and its color policy. This is a demo setup defect; no renderer regression is established. The private demo launcher now removes inherited color overrides and supplies terminal capabilities for future launches. At that point the running client had not been restarted; the successful retry above supersedes this state. Resume the same Session through a fresh native client after preserving any current input; do not create another conversation or disturb the active turn.

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

Earlier feedback state, superseded by the quiet-message direction above: worker posts plus screen 2’s compact run page, combined position/readiness and Files-owned diff remain the accepted shape.

Earlier proposed direction, now requested and implemented above: expose a concise progress post without displaying the transport envelope; retain exact evidence in history. Investigate the first report's four-minute delay and establish a working native Task opening before repeating the run-page demo. These are proposals and investigations, not an accepted redesign or proven root cause.

## Remaining proof and next action

Historical remaining-proof list: the all-provider idle wake/rendering/draft cases are now observed above; comprehensive reconnect, mounted run-page interaction, cadence/noise judgment and broader gate remain. The native draft specimen and isolated app are left available; no Session was completed or closed. No worker restart, publication or merge followed this review.

Earlier proposed next action (now performed): resume the requested native demo when the desktop is accessible. Open `/Users/jack/Applications/Loopflow LOO-711 Demo.app`, select Worker updates demo, and use Move here for the disposable existing Codex Session. Jack should type a fresh unsent draft and say ready in the ongoing conversation before a harmless fixture Graph runs. The existing specimen draft belongs to the fixture and is not expected to transfer. Preserve the first-delay and opening failures. Do not call the overall demo successful from the native delivery evidence alone.
