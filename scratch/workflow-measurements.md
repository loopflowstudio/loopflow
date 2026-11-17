# Intended live workflow measurements

“ok, keep wroking on getting all the intended workflows measured”
“not the job of this task to fix it”

Collect ordinary-use app journal observations for app launch, Task selection,
Session return, new conversation, first response, filtering, file opening,
reconnect, Start and Complete. Run against the live store on the live Machine;
keep repository/build/source and failed attempts separate. Report app-process
start separately from an external launch request if the latter clock is absent.
No latency optimization, shipping approval, or quiet-host prerequisite.

First repair: launch currently ends at sidebar rows without input readiness;
process-start records are excluded by the report. Session return has no captured
input. Task selection needs its original interaction retargeted to its Session,
including already-focused retained terminals. Then new conversation, file input,
reconnect and actual first-response visibility. Do not substitute PTY echo,
spinner, publication callback or transcript completion for a response word.

Delete the sidebar-only launch usable endpoint. Reuse the terminal focus/control
checks, actual captures and shared journal. Keep missing workflows on the charts.

Checks before live collection:
- `uv run pytest python/tests/test_desktop_timings.py -q` — 14 passed; Ruff passed.
- Decoder tests (`cargo test -p loopflow-execution --lib --features test-support harness::codex_decode::tests:: --jobs 2`, LF_* cleared) — 19 passed; `cargo fmt` and `cargo clippy --all-targets --jobs 2 -- -D warnings` passed.
- Shared/Mac interaction, launch, response visibility and file tests — 48 passed.
- Final interaction/launch/response checks after reconnect gating — 21 passed.
- Desktop build after moving launch observation to the actual focused window's terminal display callback — passed.

Live probe LOO-929 was reopened for these trials, without a Graph run. The
owned checkout contains `scratch/performance-probe.txt` as a plain text editor
fixture. Restore completion after collection. New Start/Complete timings from
its earlier run remain separate by build. New build 170cc346 observations on the live Machine: app process start → typing 131432.35 ms (75F2627B-571A-47FD-A049-C990AA8EE156), Task selection → input 507.67 ms (73C9AE98-9F2B-4F4A-9141-D7BB8FEBE74B), reconnect → typing 3030.18 ms (97BBF095-6462-4546-86DF-6067A2D64C39). Each n=1, captured-frame upper bounds. Actual unsent typing was verified and cleared. No P95 or speed comparison.

Live validation rejected three launch observations from build 0814633052: EF72F4E2-8C75-40EC-9D95-62828222AD4F, FABF453D-C86B-4FB9-AC1A-386467035ADA, E05C76AC-9DDA-4E0B-A48F-0C599204BBA0. The observer depended on system key-window activation under background app control; menus/focus could end the interval late. Raw journal records remain intact; interaction-validations.ndjson records the endpoint rejection. This is unrelated to the valid earlier 23.8 s Start sample. Launch now follows the actual input-focused terminal window.

Remaining live collection:
- Native first-response sample FA7A874F-A520-4BE3-A814-32719F32785B showed Tungsten on screen, but Session preview stayed null. Native Codex uses turn-end side reads rather than the live decoder notification path. Reuse the existing watched transcript reading to supply bounded assistant identity, then require matching visible terminal text. This remains a response-observed upper bound, including transcript publication.
- New conversation 1C421473-D781-4C20-B949-EC79AB39206B and return 43671275-24D3-4746-8DAB-138F273790AD were accessibility actions without native input timestamps. No latency claimed. Native coordinate clicks on that popover did not activate its controls. A real-click trial is still needed. The additional owned conversation is session_21785126813e4db8bfcb8b3cd50acec1 and replied without changing files.
- File trial 8CB3C9AB-DC8C-4779-9CD4-BB683C5AE69B included a driver pause before focusing the editor; rejected by sidecar. End at mounted, visible, editable contents so a second focus click cannot enter file-loading time. Repeat after installing the corrected build.
- Repeat new journeys on Manabot and Kata; retain absent repository measurements.
- Restore completion of LOO-929 after collection. Its text fixture was restored after typing/Undo.

At 2026-11-16 15:30 local the host reported load 121.09/120.53/119.44, with 20 GiB disk available. This is context, never a baseline gate. Current Machine service remains 470da475a; new native-preview projection needs the updated reader at runtime before an app sample can qualify.

“im not sure i understand what i was supposed to do” — Jack Heart, after the request for real popover clicks. Clarified: select LOO-929 in Loopflow Dev, press ⌘/, click New conversation; said this could be left for now.

Checks for the native response/file repair: 39 focused Swift tests passed; final input/response checks 12 passed; Python timing report 15 passed and Ruff passed. Native transcript/provenance Rust checks passed; final narrow assistant-only parsing check and Clippy pending. Native rollout context/user JSON is excluded before parsing so measurement does not reread every injected prompt.

Latest checks: native transcript/turn tests 9 passed; native provenance 1 passed; full Clippy and formatting passed. Swift response/file/interaction checks 39 passed; final response/interaction checks after removing the redundant availability gate 12 passed. Report checks 15 passed and Ruff passed.

“done” — Jack Heart, after the single real New conversation click. Input 3A768CDB-286F-477F-B034-F7EF30275776 at 1794872782.422018, captured acknowledgment 196.03 ms. No usable endpoint appeared within the next several minutes. New Session session_3dacba2087aa4a88943c19da150b82fc was listed detached with no terminal IDs; the visible shell remained session_21785126813e4db8bfcb8b3cd50acec1. Preserve this unsuccessful trial, never a completed percentile.

Real-click new conversation: “done” — Jack Heart after clicking the offered New conversation action. Observation 3A768CDB-286F-477F-B034-F7EF30275776 has AppKit input and a captured acknowledgment at 196.03 ms, but no usable endpoint. New Session session_3dacba2087aa4a88943c19da150b82fc is detached, without a terminal ID; the visible shell remained the earlier UX probe. Do not turn this into a successful latency figure or fix the product path in this measurement Task. A real-click original-Session return was offered next; pending.

Latest repair validation: 9 native transcript tests and 1 native-preview provenance test passed; final cargo fmt/Clippy all-targets warnings-denied passed. Swift file/interaction/response tests: 39 passed, then 12 final response/interaction tests after accepting native Enter without a stale availability gate. Python report: 15 passed; Ruff passed. The prepared source is b7e820d1fc. The control helper is building before Dev installation; the shared live service has not changed.

Dev build b7e820d1f installed and launched; its focused checks passed. The first file was 39 bytes, the fresh fixture 45 bytes. File verification is blocked by repeated native screen/AX timeouts. Finder remains readable. A two-second Dev sample is retained at /tmp/loo916-dev-hang.sample; the sampled main thread was in SwiftUI accessibility visibility updates, so no claim about the cause follows from this short sample.

The supported one-shot service override was rejected by macOS: `launchctl debug` requires root. It changed nothing; the live service remains 470da475a. Prepared /tmp/loo916-enable-reader-once (shell syntax checked), pointing at the clean, already-built b7e820d1f control helper. It configures the next service invocation only, without restarting it or changing the default installation. An administrator must run it, then this conversation can restart, verify the actual service identity, collect and restore the original service with another restart. Check identity throughout: other clients can reconcile the selected production service. Never treat a configuration receipt alone as a new runtime or completed sample.

The requested real-click Session return was withdrawn before Dev installation; no further action is owed. LOO-929 remains open until the missing collection can resume. Both fixture text files are owned by this trial and need cleanup after collection. No performance fix or merge was authorized.

Recovery pending after administrator-enabled override: “done” — Jack Heart. `launchctl kickstart -k gui/501/com.loopflow.server` returned successfully, but the service entered EX_CONFIG (78), spawn failed, with LWCR/parent bundle com.loopflow.mac. A second kickstart did not restore it. No experiment measurements were collected. The reset of debug configuration requires root; requested `sudo /tmp/loo916-restore-service`. That syntax-checked script sets the program back to /Applications/Loopflow.app/Contents/MacOS/lf, original `lf server run` arguments, and restarts the same job. Verify the original 470da475a runtime and socket readiness before any further measurement. Do not try another candidate override while recovery is pending.

Recovery verified after “done” — Jack Heart: the installed 470da475a CLI reports ready=true/listening=true, protocol 5, instance 6b911057-f564-4159-ae01-c210f09e4ebd; PID 58063 owns the original machine socket, and Desktop is connected. Both temporary admin scripts are retired as no-ops; original text is retained only in private .retired.txt receipts. No further service override is planned.

Dev UI reads work again. A fresh 45-byte file is visibly open; DA297AFE-4EEF-4AD4-ADB1-F6D32F1C8018 records native input and app-state acknowledgment but no usable/captured endpoint. Screen Recording settings show Loopflow Dev access unchanged; checking actual foreground visibility before modifying instrumentation. Asked to bring Dev to the front and leave it visible for collection.

“done” — Jack Heart after bringing Dev to the front. Five warm 39–45 byte file selections completed on b7e820d1f: 86.21, 119.68, 91.74, 89.23, 82.26 ms; median 89.23 ms, no P95. IDs: 16799131-2AAB-4245-AE72-FA4659B548BF, E0153C87-6C1C-43DC-828D-6A7DCE75F105, F637C55D-DEFB-4A24-B191-478CE760D1AD, C7A75AC7-62FB-46C4-BC68-7A519C5BAB23, 1F69C3DA-4A26-4ECC-81A6-4673542B75F2. First window 2560 px wide, then 1444 px after a manual resize. Descriptive ordinary-use data, not a controlled comparison. Contents visibly verified; fixtures unchanged.

7/10 experience categories have completed live figures. New conversation, Session return and response observed remain unmeasured. The original probe is stale/unavailable following recovery; normal Refresh has no verified input-ready endpoint yet. Leave the healthy original service in place. Native response collection needs a supported matching runtime deployment; no more launch overrides.

DABA9273-6BAB-4260-A810-7D716C08FA11 is rejected by sidecar: its launch was interrupted by the service experiment, prolonged idle time and later manual reconnect. Keep raw records. The earlier 131.43 s launch remains valid.

A normal app update concurrently selected 528ccd8ed; 470da475a continued owning the socket. Supported install-server reconciliation failed on that mismatch. Verified the old PID 58063 was now in the superseded bundle, then sent TERM to that process and kickstarted the registered job. Current status is ready=true/listening=true/error=null, instance 34e0a6e7-3c60-4b9e-b136-882ddbd99979, protocol 5, Desktop reconnected. No additional override. This supersedes the earlier restored identity.

“are tou going to generate the data or how do we get to landing” — Jack Heart.
Fresh report still has 7/10 categories with completed samples; return_session,
new_conversation and first_word have none. Regenerated /tmp/loopflow-performance.html.
The current server status listens but reports a selected-executable mismatch;
no restart or override attempted. CUA reports cgWindowNotFound for the installed
Dev app; requested reopening its window, without workflow clicks. The app inventory
contains several bundles sharing the Dev identifier, so full installed path is
required. Added original-event capture to the ordinary Session Open here/Retry
button and removed the duplicate reconnect wrapper. Focused build/check running
in /tmp/loo916-final-entry-check.log. PR #1950 is open and mergeable; current
notes-only head skips the substantive CI suites, so that is not a new validation
of these edits. No shipping approval or Graph launch.
The entry-point repair compiled successfully; `scripts/test_desktop.sh --filter
'InteractionTests|ResponseVisibilityTests|LaunchReadinessTests'` passed 12 tests
in 2 suites. No new live measurements were produced by these tests.
