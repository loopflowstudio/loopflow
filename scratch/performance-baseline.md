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
