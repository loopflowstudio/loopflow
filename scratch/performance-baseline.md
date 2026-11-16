# Native interaction benchmark

Extracted from LOO-906 at `b74196d33bd8e5d1ef1c7911bf2fc86a93f7ac9e`; base
`9cafc410eeab03d772b8cac7d641d6bd7edfd435`. The original branch and its remaining
rewrite stay intact. This Task supplies the first independent reviewable slice.

Jack Heart: “lets start with the perofrmance benchmark”; “so that we can use the
benchmark to prove each subsequent step”. Quiet mini preferred; current Mac may
be used when suitably idle. No OS upgrade or merge requested.

## Live measurements first (2026-11-15)

Jack Heart: “I think what I value more is measurements that are run on my live
store, on my live machine, and ideally they log to disk or to some sort of
perflog.” “Whenever I use the apps we're actually getting live measurements from
how things are loading when I actually interact with the app.”

“I would first want to just say: \"Okay are we able to measure the interactions
that matter in actual live usage of the app?\" Then we can drive agent-driven
usage of the app to get some initial figures because we're not going to have
historical figures to start”. “And then I would say the benchmark or the fake
world stuff can be done to drive CI tests and maybe but it's probably second”.

Priority: persist ordinary interaction observations, make them readable, then
collect agent-driven live usage. Synthetic worlds and isolated-host CI follow.
Keep loaded-host samples; host context must not add synchronous interaction work
or turn real latency into an adjusted estimate. No merge approval.

Jack Heart: “i thinknthese might heba little low level i want metrics with clear
user experiences associated that are \"all in\" times like the ade benchmark”.

The headline report must measure complete experiences from the initiating action
to the usable result. CLI, SQL, refresh and publication timings are diagnostic
breakdowns, never substitutes for the whole journey. Existing queued-callback
navigation spans do not yet meet this contract. Candidate journeys:

| Experience | Start | Finish |
|---|---|---|
| Open the app | Launch requested | Restored workspace visible and intended input accepts typing |
| Open a Task | Task selected | Correct workspace visible and usable, with its retained state |
| Return to a Session | Session selected | Correct conversation visible and its input accepts typing |
| Start a conversation | New-conversation action | Correct destination visible and composer accepts typing |
| Get an answer | Submit input | First response word visible |
| Find work | Filter/search changed | Matching list visible and selectable |
| Open a file | File selected | Correct contents visible and editor accepts input |
| Recover a connection | Return/reconnect begins | Correct conversation restored and input usable |

Measure first/cached/fresh and idle/streaming variants separately. Count unfinished,
failed and superseded journeys. Continuous ordinary-use recording needs a truthful
usable endpoint; rendered-state callbacks alone do not prove pixel or input readiness.
Agent-driven initial trials can validate visible/input endpoints with capture and
owned probes. The live report just published is a diagnostic view, not completed
all-in experience coverage.

### UX report update (2026-11-16)

Headline text/HTML now lists complete experiences and their action → usable-result
contracts; internal phase timings move under collapsed diagnostics. No old callback,
CLI or launch milestone is promoted to a full-journey observation. Existing
`agent_input.py` reports can populate Task-click → observed unsent draft response
through repeated `--input-trial` paths. Provider/case/build/Machine/data/conditions
stay separate; captured observations still need review. Other drivers and continuous
all-in app collection remain to implement. No complete input-response trial was found
in the bounded `/tmp` report locations inspected; no new native trial was started.

- `uv run pytest python/tests/test_desktop_timings.py python/tests/test_agent_input_profile.py::test_experience_report_consumes_the_full_trial_without_promoting_missing_input -q` — 7 passed.
- Ruff check/format on the three changed Python files; `git diff --check` — passed.
- Regenerated `/tmp/loo916-live-performance.html` from live journals. Browser preview
  rejected the local file URL: “The requested URL protocol is not allowed. Allowed
  protocols: \"http:\", \"https:\".” No alternate browser route was attempted.

## Controlled comparisons

Use the existing native workspace/hierarchy/Session-return and committed-write
journeys as a fixed baseline for later reactivity changes. Keep measurement
sources, fixtures, build mode and host comparable across source builds. Retain
failed/interrupted/not-started observations and require twenty warm samples for
P95. Native bitmap/OCR and owned PTY replies are distinct from compositor paint,
model publication and production provider startup. No universal speed claim.

## This slice

- Extract quiet-host preflight, aggregate SQLite profile time and 10k supporting
  projection workload without importing the reactivity architecture.
- Repair the committed-write fixture to mutate current Task rows, rather than
  the old planning JSON representation. Check its SQL against a migrated store.
- Document baseline/after commands and ADEMark/Loopflow workflow coverage. Mark
  unimplemented mobile/native journeys unmeasured. No unused signpost API.
- Verify the Python comparison/report path, focused Rust instrumentation and
  Swift compilation on this checkout. Record the load rejection separately from
  harness correctness. Publish one PR; no auto-merge.

## Delete — do not maintain

Remove obsolete `pm_items.body` writes from the native committed-write fixture.
Preserve private fixture isolation, real subscription delivery, native capture,
historical snapshot preparation and timing endpoints. No schema migration.

## Checks and observations

- Source branch Python harness: 54 passed, before extraction.
- 2026-11-16 preflight: current Mac rejected, 704.9% background process CPU and
  load averages 103.83/63.67/45.57. No native trial began. Report:
  `/tmp/loo906-benchmark-host-20261115/preflight.json`.
- Mini previously observed macOS 15.5 against Desktop's macOS 26 deployment
  target; no compatible quiet-host baseline exists yet.
- Extraction: `uv run pytest python/tests/test_desktop_performance.py -q` —
  57 passed; Ruff check/format and `git diff --check` pass.
- Extracted native SQL — rename/create/cleanup and planning revision triggers
  pass against an actually migrated disposable store. The automated Python
  check exercises the native source SQL against the released normalized schema.
- `swift build --build-tests --package-path swift --jobs 2` — passed (745.85 s).
- Opt-in `WorkspacePerformanceTests` — ran and passed, 63 observations across
  256/2048/10k Tasks; debug diagnostic on the loaded host, not a native baseline.
  `/tmp/loo916-projection-diagnostic.json`, `/tmp/loo916-projection-run.log`.
- `cargo fmt --check` and `cargo clippy --all-targets --jobs 2 -- -D warnings` —
  passed. `cargo test -p loopflow --test integration
  performance_tests::cli_volume_records_real_reads_without_contents --jobs 2` —
  passed; `/tmp/loo916-rust-test.log`.
- Mini recheck: macOS 15.5, Macmini9,1, load 1.55/1.47/1.40. No OS change made.

## Remaining coverage

The coverage table is an inventory, not a claim that every ADEMark or mobile
journey is automated. Capture the first valid native baseline before using a
later rewrite slice to claim improved responsiveness. Reuse the exact measurement
contract; changing the harness requires a new baseline on both builds.

## History audit and current gaps

- `eb5134a95e` / `9ca9495370`: navigation/input/scroll catalog and live recorder;
  a subsequent correction identifies queued callbacks rather than compositor
  completion. Keep hitches/hangs separate. Early catalog assertions were reduced;
  historical coverage descriptions must not be treated as current tests.
- `464ac18f01` (LOO-376, #1432): ordinary launch/read/refresh disk journals.
  `0ff8e90e55` (#1436): release launches against a private real Home copy.
  `c418953634` (#1454): first launch of a new binary separately from relaunch;
  ad-hoc bundle copies did not establish notarized-installation startup cost.
- `5b62453be7` and current `agent-input.md`: Claude/Codex × cached/resumed/fresh,
  exact Session input response, provider/helper timing and hook-review delay.
- `022a253f78`: phone launch-to-saved/current-list, filter, Session first screen,
  input-to-next-output. Signposts/log lines are not a persistent disk dashboard.
- Product goal / Infrastructure memory: October 7 retired numeric performance
  targets and soak acceptance during product changes. Existing report budgets
  are historical; do not reactivate them as new gates.
- DesktopVitals already emits five-minute footprint/thread/fd readings, memory
  pressure and main-thread hang observations into logs/lf.log. Do not add a second
  watchdog. No per-interaction host load or general frame-hitch distribution yet.
- ADEMark pinned source remains the README's `119a5f7b...`: Track A1–A12 and
  C2–C6 name relevant UX/resource/recovery measurements. Existing harnesses are
  partial implementations, with no qualified native baseline in this extraction.
  In particular: five-stream typing, replay-to-first-visible-word, 20-thread
  scaling, attributed footprint/energy, 2k-message frame drops, 30-minute real-work
  memory slope and native fault/recovery journeys remain gaps. Mobile remains
  partial. Fixed replay/host gates are controlled-run requirements, not reasons
  to discard ordinary-use observations.

## Live report/check observations

### First live three-repository pilot, 2026-11-16

Jack Heart: “yeah this is more reasonable. can you just get some initial numbers,
now on looplfow, manabot, and kata”.

Installed Desktop 0.16.0, source `470da475a163869b59388f932273962b4d0ba69b`,
against the live `~/.lf` Machine and `/Users/jack/src/{loopflow,manabot,kata}`.
18 native observations: 3 per experience/repository. No Task submission, prompt
submission, reset or input takeover. Existing repository conversation restoration
still occurs as ordinary app behavior when opening its workspace.

| Experience | Loopflow | Manabot | Kata |
|---|---|---|---|
| New Task → unsent request text visible | 1.313 s (1.297–1.333) | 1.346 s (1.292–1.417) | 1.644 s (1.324–1.744) |
| Backlog → Working list visible/selectable | 0.661 s (0.661–0.665) | 2.734 s (0.671–5.137) | 0.834 s (0.651–2.390) |

Values are median observation-inclusive **upper bounds**, with min–max observed
bounds in parentheses. Start is immediately before CUA native click dispatch; end
is after accessibility verification and native screenshot return. Typing the probe
and observer work remain in the interval. These are not app-only first-frame latency,
a P95, an isolation of app versus observer delay, or an optimization baseline.
Working counts checked: 12/17/8; Backlog counts displayed: 188/52/21. These are
live displayed section counts, not total historical store sizes. Host load averages
were 10.737/11.594/9.918 before and 30.180/35.149/23.667 after. No load correction.

Conversation input readiness remains unmeasured: LOO-916 was read-only with Rex
holding input; Manabot showed a held terminal/account-cooling error; Kata showed
restoration/input-paused state. No provider probe was performed. Partial Task and
repository sidebar observations are retained separately and excluded from the table.
Native menu stale-AX IDs and folder-picker selection problems were setup issues,
not measured app failures. Captures returned JPEG; extensions were corrected without
transcoding. The scripted input response and selected repository were checked in
every AX receipt, and endpoint/outlier screenshots reviewed.

Raw attempts, host/data identities, screenshots, exact CUA driver functions and
reproducible summary renderer are private on disk at
`/Users/jack/.lf/performance/live/20261116-three-repositories/`.
`report.html`, `report.md`, `summary.json`, `trials.json`, `host.json` and
`setup-issues.json` retain the figures and gaps. No raw captures enter the PR.

- 2026-11-15: local retained 0.15.3 journal has 28 launch records; saved usable
  proxy n=26, median 566.0 ms, P95 4283.4 ms. No host load context; pooled builds
  within a version. Slow-reading percentiles sample only wait+apply >=100 ms.
- Generated `/tmp/loo916-live-performance.html` from the live Machine journals;
  opened through the system file opener. New navigation journal needs an updated
  app build; it is not active in the installed app yet.
- `swift test --package-path swift --jobs 2 --filter LaunchJournalTests`: 9 passed,
  build passed. Existing warnings in ServerClient.swift and TaskWorkView.swift.
- `uv run pytest python/tests/test_desktop_timings.py -q`: 4 passed. Ruff format,
  Ruff check and git diff --check pass for the live reporting change.


## Review rework, 2026-11-16

Jack Heart: “wait no. acutally. just take on both in this PR./”

Keep the harness extraction and corrected live journal together in #1950.
Do not split the journal. No shipping approval or new numeric budgets.
The supplied review calls for original event time, separate acknowledgment and
usable presentation, shared Desktop/phone records, revision attribution, longer
retention, cheap load context, existing-watchdog stalls and attributed work counts.

This review found missing screen observations, UIKit event capture and attributed
server SQL. The completion pass below replaces those app-state-only endpoints.
Live journal trials on Loopflow, Manabot and Kata and interleaved parent/candidate
comparisons still need native capture validation.

SQLite profile callbacks have millisecond resolution even though their values use
nanosecond units (https://sqlite.org/c3ref/c_trace.html). Fast real statements may
report zero elapsed time. Assert positive callback count, not positive duration.

- `swift test --package-path swift --jobs 2 --filter InteractionTests`: 5 passed;
  shared framework and Desktop build passed, including board/filter source edits.
- `uv run pytest python/tests/test_desktop_timings.py -q`: 8 passed after parser rework; Ruff check/format passed.
- iOS simulator Debug app build passed (`/tmp/loo916-ios-build.log`).
- `cargo clippy --all-targets --jobs 2 -- -D warnings`: passed; focused CLI-volume integration test passed after repairing the missing profile increment.

Delete: the replaced phone Wait token, queued-callback completion helper, old
manual input-trial headline importer, and interaction writer in LaunchJournal.
Keep launch/read diagnostics and independent native input-capture receipts.


## Completion pass, 2026-11-16

Jack Heart: “Still incomplete: actual frame presentation, UIKit event capture and
full work attribution. Those gaps remain marked unmeasured.
 Lets get this done?”

Mac uses ScreenCaptureKit with an existing grant to observe its visible window;
its acquisition timestamp is an upper bound. Phone uses ReplayKit video-frame
host timestamps during an explicit visual-timing session, with microphone/camera
disabled. Backgrounding stops the session. No images/audio/video are retained.
The earlier UIKit hierarchy smoke check was only an offscreen-render check and is
superseded: it does not prove presentation. Terminal readiness prepares a native
draw before the screen observation. Stale/future/invalid frame clocks never pass.

The Simulator caught SwiftUI ignoring NSPrincipalClass. The explicit UIKit entry
point passed the native-action clock test; scene delegates feed SwiftUI scene phase.
Server SQL scopes cover setup and pulls, inherit workers, and assign evaluation IDs
to deduplicate shared work. Pending/missing receipts remain sqlComplete=false with
null SQL totals; counters freeze at outcome. Protocol 6 adds read/write receipts,
while protocol 5 keeps ordinary requests usable without them.

Remaining verification: final focused checks, real native screen capture and live
journal trials. A new screen-capture grant needs the person's action or confirmation
under the computer-use tool policy. No merge approval.

- `uv run pytest python/tests/test_desktop_timings.py python/tests/test_bundle_version.py python/tests/test_loopflow_dev.py -q`: 23 passed; Ruff check/format passed.
- Simulator `InteractionInputTests`: 2 passed (native event clock, disabled capture stays unmeasured). Final ReplayKit lifecycle build passed.
- Measured server read/write tests: 2 passed, including an actual disposable repository configuration write. Final Clippy/schema checks pending.
- Final `cargo clippy --all-targets --jobs 2 -- -D warnings`: passed after extending measured writes over request-journal work. `cargo fmt` and `git diff --check`: passed.
- Final shared/Mac `InteractionTests`, receipt transport and protocol mirror tests: 32 passed in 3 suites; app build passed. Schema tests: 8 passed; measured read/write tests: 2 passed.

Live Dev launch caught a startup crash in Perf.begin: App.init runs before NSApp
exists, and the window lookup force-unwrapped it. Crash report
`~/Library/Logs/DiagnosticReports/Loopflow Dev-2026-11-16-124101.ips` identifies
Perf.begin → LoopflowApp.init. The lookup now permits no AppKit application;
a focused pre-AppKit startup regression test exercises this case.

- Startup regression `LaunchJournalTests/startupIntervalCanBeginBeforeAppKit`: passed. Updated Dev bundle revision `22d6885a6504d06094d64af0e1134763374c86d5` launches against the live store.
- Live Kata Backlog → Working click, observation `E4D04DD2-0B56-4532-861D-0A106EEFEE4B`, recorded original AppKit input time and repository context. No frame was acquired; outcome remains unmeasured. This is not a latency baseline.
- System Settings → Screen & System Audio Recording has no Loopflow Dev entry. New access has not been granted. Confirmation requested under the computer-use tool policy before adding this app. No phone capture session was started.
