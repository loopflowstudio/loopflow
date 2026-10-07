# Matched Rust read-path comparison — October 6

Both variants passed **84/84 endpoints**, with one first interaction and twenty
successful warm observations for each of four scenarios. Rust Session-list and
roadmap reads improved. Neither variant meets the memory budget; native reopening
p95 worsened. This is partial performance evidence, not Task acceptance.

October 6 follow-up found that the sandbox refuses Git reads in retained sibling
checkouts. The latest 36-launch cohort includes 31 failed Git commands. These
comparisons do not establish successful existing-checkout discovery/status cost;
see [Git attribution](#git-attribution-and-fixture-read-boundary--october-6).

Jack Heart requested the missing matched comparison without another speculative
memory patch. Private receipts remain at `/tmp/loo304-matched-20261006/` in
`baseline/` and `candidate/`. [Aggregate measurements](summary.json) retain receipt
hashes, binary/source identities, first observations, distributions and counters;
no copied records, pixels or credentials are published.

## Comparison contract

Baseline Rust source: `50107313717557a1a2d44adbbcd7150b7d16a990`, archived at
`/tmp/loo304-baseline-source-501073137`. Candidate: `3c931cc4d8aff396770b97d42fce032248bac7d2`.
Only common CLI instrumentation and the fixture index were added to the baseline.
The prerequisite hashes match both trees. The index exists on both sides; baseline
still reads complete Exec membership. No historical rows or app optimizations
were backported. The compared production paths are `engine/git.rs`,
`engine/process.rs`, `ops/human_session/workspace.rs`, `ops/task/lifecycle.rs` and
`store/sqlite/task_work.rs`. The installed-binary digest cache is outside this
custom-Home execution path and receives no proof from these results.

Both CLIs were freshly built with the same Cargo dev profile (`debug=1`,
`incremental=false`), sharing compiled dependencies. Current Swift, observation
harness, contained native fixture and published GhosttyKit lf3 were constant.
SwiftPM debug `-gnone` tests were built before measurement; the runner's repeated
builds were no-ops. Both runs used the identical test executable hash. Builds
finished before the sequential baseline/candidate pair; no other heavy benchmark
was launched. A read-only source archive audit overlapped baseline sampling.

Both used `/tmp/loo304-compatible-snapshot-20261006-02`: all 39 tables, 342 Tasks,
604 Sessions and 218,637 Execs. Its checksum still matches. Source, binary and
authored configuration hashes stayed unchanged. Copied Sessions remained
observational; only the separate owned Task/native fixture executed. The existing
sandbox retained credential, network, provider and external-write restrictions.

Host: Apple M4 Max, macOS 26.0.1. One-minute load median/max was 14.8/16.9 for
baseline and 14.5/20.3 for candidate; the ordered pair is not randomized and host
load remained variable. Minimum free space was 13.0/11.7 GiB, above the 6 GiB
reserve. No trace, store or unknown output was removed.

## Warm observations

Every cell is median / nearest-rank p95 milliseconds, n=20. First observations
remain separate in the JSON and are not folded into these distributions.

| Scenario | Baseline bitmap-ready | Candidate bitmap-ready | Baseline OCR-inclusive | Candidate OCR-inclusive |
| --- | ---: | ---: | ---: | ---: |
| Cold workspace construction | 1407 / 1678 | 1283 / 1572 | 1704 / 1999 | 1548 / 1841 |
| Warm Task | 168 / 195 | 153 / 191 | 447 / 568 | 400 / 497 |
| Reopen Task | 46 / 55 | 41 / 51 | 355 / 438 | 301 / 369 |
| Native Session reopen | 1123 / 1305 | 1076 / 1562 | 1469 / 1704 | 1409 / 1854 |

Bitmap-ready ends when the successful image was captured; OCR subsequently
establishes exact identity. It is not compositor presentation. Earlier failed
capture/OCR attempts can already be included before that endpoint. No observer
time is subtracted from action duration. Native capture itself had median bitmap
cost 112/105 ms and OCR cost 326/325 ms per capture. The JSON retains equivalent
breakdowns for all scenarios. These observations do not measure cold application
launch, first frame or one-frame key-to-glyph latency. The warm-Task bitmap p95
still exceeds 100 ms; its OCR-inclusive p95 is 497 ms. Budgets are unchanged.

All 168 recorded Task-opening window transitions showed one visible window;
key-window identity was `-1` throughout, so real key-window continuity remains
unproved. All 21 native rounds per variant preserved focus/surfaces, pane layout,
drafts, selection, transcript and native identity. No endpoint failed or timed
out. Read failures remain: each side had 42 refused copied Task-comment reads,
one refused owned-fixture comment read and twenty canceled activity reads.

## Real read and resource costs

These are transport read durations, including adapter work and, where applicable,
the owned fixture read. They are not individual process startup measurements.

| Read | Baseline n; median / p95 ms | Candidate n; median / p95 ms |
| --- | ---: | ---: |
| Session list | 65; 2335 / 3156 | 89; 705 / 1384 |
| Roadmap | 52; 5140 / 7519 | 52; 3759 / 5309 |
| Task status | 43; 1336 / 1673 | 43; 1164 / 1447 |
| Wave list | 30; 1051 / 1562 | 30; 937 / 1588 |

Session-list median fell 70%, roadmap 27%; both remain above 300 ms. The aggregate
JSON includes other reads and failures. CLI starts/ends were 495/495 → 543/543;
SQLite connections 2,817 → 3,057, statements 247,495 → 259,358, emitted rows
2,973,494 → 3,097,988. Counts include both scenario Homes, exclude fixture setup,
and exclude Git/provider processes and non-main-store SQLite connections.
Statements include PRAGMAs/writes/triggers; emitted rows are not rows scanned.
Faster Session reads permit more refreshes within the interval: this is **not**
evidence of reduced total process/query volume or a quiet workspace.

Each variant requested 150 seconds of RSS/signpost recording and retained 150 CPU
samples. App CPU median/p95/max was 1.2/31.8/101.7% → 1.2/19.4/83.0%, excluding
children. Completing all 21 native repetitions extended the navigation interval
to 190/194 seconds; external CPU/RSS sampling does not cover that tail. Exact
in-process fourth-round growth was **58.9 → 45.8 MiB**, both failing <32 MiB.
The prior +54.5 MiB failure remains intact. This pair supports neither a production
leak diagnosis nor a successful memory repair.

## Remaining work and disposition

- Rust steady-state attribution now has a matched rendered pair. The release-profile follow-through below resolves that measurement gap,
  repairs one demonstrated query-plan regression and names the remaining costs.
  Neither pair establishes the production latency ceiling.
- Swift attribution remains missing. Candidate Swift was intentionally constant,
  including direct-open routing, joined Session reads, projection changes and
  unchanged-error publication suppression. The historical Swift tree lacks the
  current combined fixture interface; building a valid full comparison requires
  separating observation/launch/font prerequisites from those production changes.
  The Rust pair must not be reported as proof of all Swift improvements.
- Capture-related allocation pressure remains the measured memory bottleneck.
  Buffer reuse and autorelease scope already failed and were removed. Equivalent
  pixel capture comparison or targeted AppKit/Vision allocation attribution is
  the actionable next diagnostic; no speculative production patch is required.
- Both 3,600-second matched soaks and full hitch/hang coverage remain missing.
  RSS-only recording has no hitch/hang tables or trace clocks. The retained
  minimal Instruments probe projects about 94 GiB raw storage per hour; current
  free space is 11.7 GiB with a 6 GiB reserve. No verified lower-volume equivalent
  or configured remote host exists. Repeating that capture would exhaust storage;
  neither this short run nor a tail-only trace can satisfy hour acceptance.

Review retained the worsened native p95, unequal read counts, partial resource
coverage and endpoint distinctions. Existing contained proofs remain applicable;
this pass changed no production or measurement code. Checks: both Cargo builds
and Swift test build PASS; sequential matched runner PASS 84/84 each, comparison
available; memory FAIL both; full-hour/stall acceptance unavailable.

## Release-profile follow-through and query repair — October 6

Jack Heart requested identical release builds of the historical baseline and
candidate, followed by a measured repair if reads still exceeded 300 ms.
[Release measurements](release-summary.json) retain all three runs and the
alternating direct-read check. Private receipts are in
`/tmp/loo304-release-20261006/`; the earlier debug pair was not repeated.

The baseline archive remains `501073137` plus the same measurement hooks and
fixture index. A fresh archive audit found no backported production optimization.
Candidate source is `3a5ea2ffe19ce02d4373afecda4f179db14268a7` (production-identical
to the debug candidate). Both used Cargo release defaults: opt-level 3, no debug
info or incremental compilation, no LTO, 16 codegen units. No profile/Rust flags
were inherited. Candidate Swift, harness, published lf3 and the compatible
39-table fixture were unchanged. Both builds completed before measurement.
The original fixture hash still matches; no schema, copied identity, live Home,
provider authority or containment change was needed.

Both original release variants passed **84/84 endpoints**, including 21 native
preservation rounds. Each scenario has one separate first observation and twenty
successful warm observations. The later repaired run passed **83/84**: its last
native reopen timed out, leaving nineteen successful warm native observations
and no native p95. Its Task-opening scenarios each retain twenty warm successes.
The failed comparison is not promoted to complete by the successful read check.

The table separates all-repository roadmap from exact Task lookup; the earlier
debug report's aggregate roadmap category combines them. These are transport
latencies, including adapter/owned-fixture reads, not single-process startup.
Cells are n; median / nearest-rank p95 milliseconds.

| Read | Historical release | Candidate release | Repaired release (incomplete journey) |
| --- | ---: | ---: | ---: |
| session list | 74; 1733 / 3099 | 90; 539 / 1028 | 88; 989 / 1453 |
| roadmap --all | 30; 4800 / 11726 | 31; 3782 / 4595 | 32; 2485 / 4017 |
| roadmap --task | 22; 516 / 928 | 22; 613 / 817 | 21; 794 / 1076 |
| task status | 43; 1125 / 2571 | 43; 1084 / 1229 | 43; 1090 / 1446 |
| wave list | 30; 862 / 1440 | 31; 970 / 1337 | 32; 1191 / 1971 |

The host also had other workers, including LOO-366. One-minute load median/max
was 20.6/46.5, 15.8/28.1 and 21.3/28.4 respectively. Sequential timing differences
alone do not establish causality. All read budgets remain 300 ms; these transport
reads fail it. Source/config/CLI hashes stayed stable, and all three runs used the
same test executable and measurement-source hashes.

### Supported cause and effect

A separate candidate stack sample placed 1,588 of 2,337 main-thread samples in
`read_task_work`'s Exec query, mostly SQLite page reads. Bundled SQLite **3.53.2**
selected `SCAN ae USING INDEX sqlite_autoindex_execs_1` to supply UNION order,
scanning 218,637 retained Execs despite the unfinished predicate. Python SQLite
3.50.4 selected the partial index, obscuring the cause in the first diagnostic.
Use the actual bundled engine for query-plan evidence. An experimental covering
index in a separate diagnostic copy was not adopted.

The unfinished checkout arm now explicitly selects the existing `execs_unfinished`
index (118 unfinished rows in this fixture). Full history keeps its original
query. No index/schema addition or fixture mutation is involved. The regression
adds 2,000 completed Execs, verifies the unfinished identity, and bounds SQLite VM
steps independently of wall-clock load. Existing membership tests preserve
Sessions, Flows, explicit binding, missing checkout history and authority.

After the rendered attempt, fresh fixture copies supplied an alternating
candidate/repair CLI-only check: one first plus twenty warm reads per path/variant,
all successful, same sandbox and counters. This excludes the Swift adapter and
owned fixture, so it must not be substituted for the transport table or UI time.

| Direct CLI | Before median / p95 ms | After median / p95 ms |
| --- | ---: | ---: |
| roadmap | 4334 / 6777 | 1910 / 2874 |
| sessions | 309 / 437 | 315 / 406 |

Roadmap median fell 56%; Session-list timing was effectively unchanged. Roadmap
load medians were both 34.8 (max 57.6/56.3); Session medians both 21.0. This measured
effect agrees with the bounded-query regression and bundled plan, but the 300 ms
budget remains unmet. Post-repair sampling reduced the Exec-query branch to
41/1,035 main-thread samples. Remaining concrete costs were `flow_exec_ids`
(341/1,035) and repeated repository/Git discovery. Next: profile that exact
Flow-membership join with bundled SQLite and batch same-repository discovery
without weakening process/Flow settlement. Short Session samples do not identify
a single dominant query; isolate startup/transport and refresh duplication next.

### Retained failures and acceptance limits

The repaired run's final native round showed “Not running here.” Round 19 had
closed its surface; round 20 emitted no new `session connect` read before timeout.
The next diagnostic is Desktop's cached readiness/reopen/refresh transition before
launch, not a speculative provider or memory patch. Its final pixels, journal,
owned database and prior successful rounds remain private and intact. No retry
was used to erase this failure. Twenty preservation rounds passed; the last did not.

Fourth-round growth failed in all three runs: **+60.9/+53.5/+47.6 MiB**. Earlier
+58.9/+45.8 and +54.5 MiB failures remain. App CPU median/p95/max was
0.7/21.6/88.7%, 0.8/35.3/92.8%, and 1.1/47.7/98.1%, with 150 samples per run,
excluding children. Recording covered 150 seconds; successful baseline/candidate
navigation lasted 184/186 seconds. The repaired journey has no successful end.

CLI starts/ends were 509/509, 550/550 and 545/545; statements
251,304/264,672/267,064; emitted rows 3,017,860/3,165,218/3,196,622. Counts exclude
setup, Git/providers and other SQLite connections. Faster reads did not establish
quietness. All recorded Task-opening transitions retained one visible window;
key-window identity remained unavailable. Refused comments and canceled activity
remain reported. Warm bitmap/OCR distributions, first observations and CPU/counter
limits are in the JSON. Bitmap-ready is not compositor presentation; no time was
subtracted for OCR. The 100 ms interaction budget remains unmet.

Both full hours, hitch/hang/one-frame glyph evidence and independent Swift
attribution remain open. No 94 GiB/hour trace was attempted; measured storage still
cannot safely hold it. No historical trace/store was removed. Review retained the
worse Session/UI observations, missing repaired-native p95 and failed end state.
Release's current installation/publication memory supplies no Desktop acceptance;
older release incident sections were not reread in this pass.

Checks: release builds PASS; Task-membership suite PASS 4; cargo fmt and all-target
Clippy PASS after fixing a test-only clone warning; original release pair PASS
84/84 each; repaired journey FAIL 83/84; alternating reads PASS 84/84. Publication
is authorized with these limits; neither merge nor Task completion is requested.

## Native reopen diagnosis — October 6

Jack Heart requested a focused repair of the final failed reopen, with publication
only. The original `repair/` receipt remains **83/84**, with **nineteen successful
warm native observations and no native p95**. Its pixels and failed memory result
are unchanged.

The contained reproduction refreshes Session records while the owned client is
running, lets that client exit, then opens its Task link before the next poll.
The sandbox prevents inspecting the running provider client, so the list reader
returns a closed record with `Session client observation unavailable` and disabled
actions. Desktop cached that reason. `SessionsStore.recover` silently returned
before calling connect, even though the client had since exited. A later poll
cleared the reason without resuming the selection: “Not running here.”

`/tmp/loo304-reopen-refresh-before-20261006/` reproduces this on the first warm
reopen, with one successful initial endpoint. Request/state receipts show the link
was consumed, the state stayed pending and no second connect read ran. This rules
out a lost link notification in that reproduction. The prior stale-*active*
hypothesis was contradicted by the fixture's actual closed/unknown observation.
An ordinary focused run without the ordered refresh passed 21 endpoints;
`/tmp/loo304-reopen-before3-20261006/` preserves it. Two earlier focus-only setup
attempts and the rejected active-state assertion remain in adjacent private
`loo304-reopen-*` directories.

Explicit selection now revalidates a cached observation failure through the
existing `session connect --json` operation. A fresh failure remains visible;
a returned unavailable action cannot prepare a surface. Opening/prepared/live
states still coalesce, active-elsewhere selection still requires explicit Move
here, and no `--replace`, `--try`, provider retry or deadline was added. Rust,
public lf3, snapshot contents and the containment policy are unchanged.

The regression uses the combined soak's existing native transcript/input,
companion pane, layout, file draft/selection and identity checks. Its explicit
close notification previously addressed an unused checkout-level Session store;
it now addresses the same repository-level store as the UI. No assertion or
measured endpoint was removed. The focused test additionally orders a real
refresh before each exit and is available through `verify-fixture --mounted`.

The first repaired realistic replay (`/tmp/loo304-reopen-refresh-after-20261006/`)
passed five endpoints, then timed out in **Opening session**, rather than the
original pending state. The failing capture took 3,086 and 1,874 ms in OCR while
host load was above 50. The original five-second endpoint remains intact. A
teardown broken pipe is retained in its log; this is failed evidence, not a
successful 21-round receipt. These diagnostic runs use SwiftPM debug builds and
supply no causal latency comparison or replacement performance distribution.

A second realistic focused replay (`/tmp/loo304-reopen-realistic-final-20261006/`)
passed **20/21** endpoints. The final request completed connect and reached live
surface state, then missed the native readiness endpoint. This supplies nineteen
warm successes, not a native p95. That receipt did not distinguish focus, selected
Task and visible native history; the regression now records those predicates
separately. Its failure cleanup also drains outstanding reads before exiting.
The remaining investigation is post-connect readiness and observer/main-actor
competition under load, not the now-reproduced stale-action gate. These failed
realistic runs remain part of acceptance; a small contained pass cannot erase them.

Final source checks: `swift test --package-path swift --jobs 4 --filter
SessionsStoreTests` passed 11 tests (including three cached/fresh-result cases).
`verify-fixture --mounted` passed five contained tests, three production shell
launches and **21/21 native reopens**, including all final-round preservation
assertions, at `/tmp/loo304-reopen-contained-final-20261006/`. Runner pytest passed
42 tests; Ruff and diff checks passed. The prepared realistic snapshot's SHA-256
still matches its manifest. No matched p95, 32 MiB growth, full-hour, hitch/hang or
compositor acceptance follows; publication remains separate from merge/completion.


### Readiness before observation — October 6

Jack Heart requested diagnosing the post-connect failure without changing its
five-second endpoint. The separated-predicate replay at
`/tmp/loo304-readiness-before-20261006/` passed **21/21** realistic reopens on
`6dd8f171f`. Selection remained correct; surface, retained native history and
focus became ready together. The historical last-round failure did not reproduce.
Its missing predicates cannot be reconstructed from its final screenshot.

The replay exposed observer interference: every reopen performed three synchronous
main-actor bitmap/OCR passes, including passes while connect/mounting was pending.
The historical failed attempt spent roughly six seconds in capture/OCR. That is
a demonstrated source of main-actor obstruction, but does not identify which
historical readiness predicate missed its deadline.

Native reopening now waits for its independent selection/focus/history predicates
before capturing pixels. Capture, both unchanged OCR recognizers, the second
readiness check and input proof remain mandatory. The same five-second readiness
deadline includes capture/OCR, with an explicit check after capture so a slow
observer cannot turn an expired endpoint into success. Pixel-dependent catalogue
journeys still capture before checking labels. Transition-only readiness records
and monotonic journal timestamps separate application readiness from observation
without hundreds of synchronous journal writes. No production readiness, timeout,
provider, snapshot or sandbox code changed.

`/tmp/loo304-readiness-after-20261006/` passed **21/21**, including every transcript,
input, pane/layout, draft/selection and identity check. Each reopen needed one
capture. Both focused runs use the same release CLI, published lf3, prepared
snapshot (604 Sessions, 218,637 Execs), SwiftPM debug configuration and ordered
refresh-before-exit sequence; the observer differs intentionally. The original
fixture SHA-256 still matches `d9dc08b5d7d241294b78bf5035d47e8e9069aa58c13808d04d31cd8a0f8230e4`.

| Focused warm native observation (20 successes each) | Before median / p95 ms | After median / p95 ms |
| --- | ---: | ---: |
| Application predicates, before capture | unmeasured | 596 / 1053 |
| Bitmap-ready, including preceding work | 817 / 973 | 694 / 1177 |
| Total OCR per reopen | 703 / 1281 | 266 / 386 |
| Endpoint plus input proof | 1063 / 1639 | 974 / 1511 |

These nearest-rank p95 values describe these focused runs only. Unequal host load
and the intentionally changed observer prevent attributing a product speedup;
bitmap-ready p95 worsened. No CPU/hitch/hang recorder or hour soak ran in these
focused probes. Their RSS journals are not memory acceptance. Earlier **83/84**,
**5/21** and **20/21** failures, nineteen-warm-native cohorts and failed memory
results remain unchanged. A current pass cannot prove the historical failure's
exact cause. The full matched comparison needs this observer on both sides;
all original budgets, compositor and full-hour obligations remain open.


The subsequent full candidate sequence at
`/tmp/loo304-readiness-full-20261006/` passed **84/84**, including all 21 native
rounds and final preservation assertions. It used the normal runner's SwiftPM
`-gnone` build, real snapshot reads and production refresh owner:

```bash
uv run python scripts/desktop_performance.py run \
  --snapshot /tmp/loo304-compatible-snapshot-20261006-02 \
  --lf /tmp/loo304-release-20261006/bin/repair-lf \
  --repo /Users/jack/src/loopflow --issue LOO-368 --samples 21 \
  --output /tmp/loo304-readiness-full-20261006 --no-xctrace
```

| Full candidate endpoint (20 warm successes each) | Median / p95 ms |
| --- | ---: |
| Cold-workspace construction | 1809 / 2489 |
| Warm Task | 453 / 553 |
| Task reopening | 345 / 432 |
| Native reopening plus input | 1237 / 1592 |

First observations were 2937/578/421/1353 ms respectively and are excluded from
these warm distributions. Native application predicates preceded capture at
788/1034 ms median/p95. These observations still miss the original budgets.
One visible window and terminal focus were retained; key-window identity remained
unavailable. Source, CLI and authored-config hashes stayed unchanged. CLI starts/
ends were 405/405, with 2,332 SQLite connections, 200,424 statements and 2,365,177
rows. There were 43 refused comment reads (42 copied, one owned fixture) and 20
cancellations. They remain explicit gaps, not successful provider-backed reads.
CPU, stalls, hitches and hangs were unmeasured: no recorder or soak was requested.
RSS checkpoints do not establish a four-round memory pass. This candidate-only
replay verifies the repaired observation workflow, not the full matched comparison.

Review kept the post-capture deadline check and rejected a speculative production
focus patch. Checks: Swift build PASS; full realistic sequence PASS 84/84; focused
before/after PASS 21/21 each; SessionsStoreTests PASS 11; headless verify-fixture
PASS 5 (`/tmp/loo304-readiness-contained-20261006/`); diff PASS. No merge or Task
completion follows.

## Flow membership and repository discovery repair — October 6

Jack Heart requested a bounded repair of the remaining roadmap read cost, followed
by publication to PR #1460 without merge or Task completion. The final observer
and its successful 84/84 replay above are unchanged. This pass measures direct
CLI reads; it does not replace that replay or establish rendered acceptance.
[Read measurements](flow-read-summary.json) retain all 84 samples, binary/source
hashes, counters, preservation checks and private receipt hashes. Raw outputs,
profiles and reproducible scripts remain at `/tmp/loo304-flow-read-20261006/`.

A fresh pre-change roadmap profile reproduced the old lead: Flow membership took
498 of 2,479 main-thread samples; portfolio validation and Wave display also
repeated repository discovery. Bundled SQLite 3.53.2 chose `SCAN e` for the
Session-event join, scanning 521,637 events for each Flow. Selecting matching
Sessions with `IN (SELECT id …)` instead uses `session_events_history`. Across all
104 Flows, both queries returned the same 495 Exec memberships. Total VM steps
fell 163,669,123 → 1,942,478; measured query CPU was 18.366 → 0.276 seconds.
This diagnostic exercises every Flow, not the smaller current-roadmap subset.

The production query preserves event, driver and mechanical membership, deduplication,
closed history and causal-parent exclusion. No schema or fixture index changed.
A second small change shares repository resolution between validation and display
within one command. Each subsequent command resolves fresh paths; configuration,
portfolio validation, aliases and missing-path fallback retain their existing rules.

### Matched release reads

The retained baseline binary is the preceding partial-index repair (`9aa94bfab`),
whose Rust/Cargo sources are identical to this contribution's starting `c12aae56d`.
Its hash matches the earlier `repair-lf` receipt. The candidate was built with the
same release defaults and no profile/Rust flag overrides. Both variants used fresh
copies of the unchanged compatible snapshot: 39 tables, 342 Tasks, 604 Sessions,
218,637 Execs. No live Home, copied execution authority or provider was used.

All builds finished before measurement. Order alternated for each sample, using
the same sandbox, CLI instrumentation and Git Trace2 receipts. One first read is
reported separately; cells below are median / nearest-rank p95, **20 warm successes
each**. All **84/84** reads passed, without timeouts.

| Direct CLI measurement | Before | After |
| --- | ---: | ---: |
| Roadmap elapsed, ms | 1135 / 1922 | 602 / 812 |
| Roadmap child CPU, ms | 1067 / 1310 | 576 / 663 |
| Roadmap Git processes per read | 137 / 137 | 67 / 67 |
| Roadmap `--git-common-dir` processes | 119 / 119 | 49 / 49 |
| Session list elapsed, ms | 237 / 245 | 239 / 248 |
| Session list child CPU, ms | 231 / 237 | 233 / 242 |

Roadmap median fell 47%, p95 58%; the **300 ms budget remains unmet**. Session
listing stayed essentially unchanged and was below 300 ms on both sides in this
cohort. First roadmap reads were 2412/1103 ms, Session reads 240/236 ms. Roadmap
load median/max was 17.6/19.0 on both sides; Session load medians were 17.5/17.4.
Earlier 1910/2874 ms results had different load and must not be used as this
comparison's baseline. Child CPU includes the command's waited-for children,
not Desktop CPU, interval utilization or app memory.

Each side recorded 42/42 CLI starts/ends, 315 SQLite connections, 65,711 statements
and 1,093,976 emitted rows. Statement and returned-row counts are unchanged: the
repair reduces scanned work and Git launches, not record density or endpoint
coverage. These figures do not establish idle Desktop quietness.

Session responses matched exactly. Roadmap differed only in fresh `generated_at`,
condition observation times and corresponding evidence ages (525 age checks).
Every pre-existing row, row identity and storage type across all 39 tables matched;
each copy appended exactly 42 inspection Execs. Snapshot/config hashes stayed
unchanged. The three historical unknown Execs were untouched. The initial private
measurement script used `/tmp` instead of canonical `/private/tmp` for its exact
executable allowance; sandbox launch failed before any CLI receipt. That failed
attempt remains intact, and the successful pair has a separate directory with
canonical paths and the unchanged containment policy.

### Remaining disposition

A short candidate profile retained 306 main-thread samples: Flow lookup appeared
in seven; distinct repository resolution in portfolio validation in 85; Task-work
reads in 40; pending-turn checks in 18. Sampling began after process launch and
covers different durations/load from the earlier profile, so these counts are
leads, not an end-to-end time decomposition. Next useful work is resolving distinct
linked/retired repository paths without serial Git subprocesses and attributing
per-Task membership/pending-turn queries with the bundled engine. Preserve the
existing fallback, freshness and authority counterexamples when selecting either.

Review rejected persistent repository caching and dropping historical membership.
Focused checks passed: five Task-membership tests, fourteen Wave tests, four
Task-work matches plus the historical-uncertainty acceptance regression; formatting,
all-target Clippy and release build passed. Early new-fixture failures (missing
required fields and macOS canonical-path expectations) were corrected in tests;
no production precondition was relaxed. No new whole-tree gate is claimed.

App CPU/RSS, window/focus, main-thread stalls, compositor, one-frame glyphs and
hitch/hang endpoints were not measured here. Historical failed memory results,
full matched Swift/observer attribution, cold startup and both full-hour soaks
remain open. No high-volume trace ran; its capacity blocker remains. This bounded
read repair is verified for publication only.

## Pending-turn history repair — October 6

Jack Heart requested the next measured read repair, a realistic Desktop replay and
publication to PR #1460 without merge or Task completion. This contribution changes
only the shared pending-turn query. Repository discovery was inspected but left
unchanged: replacing Git would need separate proof for its layouts and configuration.
[Measurements](pending-read-summary.json) retain distributions, counters, preservation
and receipt hashes. Reproduction scripts and raw evidence are at
`/tmp/loo304-pending-read-20261006/`.

Bundled SQLite 3.53.2 selected the full Session-history index for started turns and
rescanned retirement receipts for each turn. The query now selects the released
`session_event_input` partial index and checks Session retirement once. Completion
still matches the exact provider thread/turn; missing identity, unknown boot time,
the inclusive boot boundary and explicit retirement retain their previous meaning.
No migration, cache, membership change or process mutation was introduced.

Read-only comparisons returned identical answers for all **604 Sessions**. With
unknown boot time, VM steps fell **4,632,584 → 145,735**; with the current boot
boundary, **3,554,064 → 58,480**, query CPU **192 → 2.3 ms** across all Sessions.
These diagnostic totals cover more Sessions than a single roadmap read.

### Alternating release reads

Both normal release binaries finished building before measurement. The baseline
is the retained candidate from the preceding repair, matching this contribution's
starting `ef5ca8879147d46130850ea72a5fe5c665a09b77`. Identical isolated copies of the
unchanged compatible snapshot, containment and instrumentation supplied **84/84**
successful reads, no timeouts. Each cell is median / nearest-rank p95 with **20 warm
successes**; first reads remain separate in the JSON.

| Direct CLI measurement | Before | After |
| --- | ---: | ---: |
| Roadmap elapsed, ms | 586 / 611 | 570 / 1033 |
| Roadmap child CPU, ms | 564 / 582 | 546 / 580 |
| Session list elapsed, ms | 233 / 243 | 234 / 243 |
| Git processes per roadmap | 67 / 67 | 67 / 67 |

The median reduction is small, and **roadmap p95 worsened**. Candidate samples 12
and 17 took 1978/1033 ms with 580/602 ms child CPU; their wall-time causes remain
unknown. Paired median elapsed/CPU deltas were both approximately −11 ms. Roadmap
load median was 17.2 on both sides, maximum 29.7; Session load median was 25.9.
The earlier 602/812 ms cohort is not this comparison's baseline. The 300 ms
roadmap budget remains unmet; no tail-latency improvement is claimed.

Each side retained 42 starts/ends, 315 connections, 65,711 statements and 1,093,976
returned rows. Sessions matched byte-for-byte; roadmap differed only in fresh
timestamps and 525 verified corresponding evidence-age checks. All pre-existing
rows, identities and storage types across 39 tables survived; each copy appended
42 inspection Execs. Snapshot and authored config hashes stayed unchanged.
Historical unknown Execs and the installed Home were not mutated.

### Same-observer Desktop replay

The normal snapshot runner then ran before/after **sequentially**, 21 samples per
scenario, `--no-xctrace`, no soak. Both passed **84/84**, including all 21 native
preservation rounds. Swift test-binary and measurement-source hashes were identical;
CLI/source/config hashes stayed unchanged during each run. The baseline's early
navigation overlapped the retained-row audit, limiting timing attribution. These
runs connect the new CLI to the existing UX, not to a proved end-to-end speedup.

| Desktop endpoint, 20 warm successes, median / p95 ms | Before | After |
| --- | ---: | ---: |
| Workspace construction, bitmap/OCR included | 1346 / 2123 | 1089 / 1391 |
| Warm Task, bitmap/OCR included | 337 / 453 | 300 / 370 |
| Task reopen, bitmap/OCR included | 301 / 432 | 292 / 396 |
| Native reopen, endpoint/input | 868 / 914 | 894 / 976 |
| Native readiness before capture | 613 / 644 | 648 / 698 |

Native latency worsened. Every native round retained history/native identity,
input replies, panes/layout, file draft/selection and one window; focus reached
`GhosttyMetalView`. Process counts rose **407 → 415**, statements **204,437 →
209,230**, returned rows **2,387,165 → 2,423,759**. Faster polling is not quietness.
Each side retained 43 unavailable comment reads (42 copied Task, one owned Task);
activity cancellations were 10/2. No unavailable data was synthesized.

First/last attempt-end RSS was **239/315 MiB** before and **230/312 MiB** after.
Those endpoints span the entire sequence and are not the four-round growth metric.
Prior failed <32 MiB results remain. App interval CPU, main-thread stalls, hitches,
hangs, compositor paint and one-frame glyph latency are unmeasured here. Workspace
construction is not the original cold-start/first-frame endpoint. Both full-hour
soaks, full Swift attribution and every original budget remain open. The measured
~94 GiB/hour raw-trace requirement still exceeds available storage; no trace was
started or deleted.

A fresh candidate profile retained 418 main-thread samples: repository validation
97, checkout execution-boundary discovery 51, Task-work reads 51 and pending-turn
reads three. Trace2 still records 49 common-directory launches per roadmap. Next:
reduce repeated discovery through the existing resolver without changing canonical
aliases, missing/retired paths or authored configuration validation, and attribute
the remaining Exec membership read with bundled SQLite before changing it. Sampling
starts after launch; these counts are leads, not an end-to-end time decomposition.
The immediate sampling attempt returned 255 without a trace; a separate attempt
attached after 150 ms and succeeded. Both receipts are retained under `profile/`
and `profile-delayed/`.

Review retained exact retirement/boot semantics and rejected equating query work
with UI latency. Focused tests passed seven Task-work/query cases plus the historical
uncertainty regression; normal release build, formatting and all-target Clippy
passed. The initial new fixtures failed on a missing required `cwd`, then were
corrected; those logs remain. Publication is a verified bounded repair, not full
Task acceptance.


## Recorder capacity and coverage — October 6

Jack Heart requested resolving storage before the original matched hour pair.
Two sequential ten-second probes attached only to owned `/bin/sleep` children,
using the existing guarded recorder and Xcode 26.0 (17C52)'s Blank template.
Private commands, exact observed raw paths, exit receipts and exported TOCs remain
at `/tmp/loo304-recorder-capacity-20261006/`; neither probe is Desktop acceptance.

| Instrument | Last observed raw bytes | Actual trace interval | Linear GiB per requested hour |
| --- | ---: | ---: | ---: |
| Hitches | 39,434,424 | 10.436 s | 13.2 |
| Hangs | 307,205,320 | 10.672 s | 103.0 |

These are last descriptor observations, not guaranteed final sizes or steady-state
rates. Different process activity and tracing overhead limit extrapolation.
Hitches exports frame-lifetime/render tables but no potential-hangs table; Hangs
exports potential-hangs/runloop events but no hitches table. Dropping either loses
required evidence. The earlier combined 30-second probe's ~94 GiB/hour remains;
this decomposition identifies Hangs as the larger cost in these probes, not a
faithful replacement configuration. The local template already disables waiting-
thread/kernel-stack/context-switch sampling and priority-inversion detection.
The CLI exposes instrument selection and a tail window, but no raw collection
filter or lossless streaming option was found in its supported command interface.
A tail window would discard coverage. Only the root volume is mounted.

Initial free space was 11.9 GiB; no files were reclaimed or historical traces
changed. A faithful method must fit the unchanged 2 GiB consumption allowance
(less than 0.57 MiB/s averaged over an hour, including output overhead), retain
both tables, actual trace clocks, CPU/RSS and window/focus evidence, and establish
compositor/input endpoints. Present probes do not establish the last endpoints.
At observed rates, an uncompressed full trace needs roughly 94–103 GiB for one
hour, plus 6 GiB reserve and export/workspace headroom; retaining both raw hours
would require roughly 188–206 GiB before that headroom. These are provisioning
estimates, not a proved minimum or permission to raise the guard. A capable
recorder needs verified lossless lower-volume collection/storage, not shortened
sampling or an unverified external output path (TMPDIR previously did not relocate
raw capture). Neither full-hour run was launched.

Inspection found an independent coverage defect. Real Xcode exports put clocks
under `run/info/summary`; the reader expected `run/info/run-info`, matching only
its synthetic fixture. The reader and regression now use the real structure;
both retained probe TOCs decode their exact dated intervals. Reports expose
full-soak opening/closing gaps. Trace-enabled runs require a successful recording,
both hitch/hang tables and coverage of the entire journal interval before they
can report complete or supply a matched comparison. Missing resources remain
unmeasured. RSS-only diagnostics retain their distinct mode and establish no
trace acceptance. No historical report was overwritten.

The current harness starts recording after `soak_begin`; a startup barrier tied
to actual Instruments readiness is still needed, followed by coverage through
`soak_end`. This report repair detects that gap; it does not close it. The original
baseline remains `501073137` plus documented common prerequisites, compared with
the current candidate under identical observer/build profiles. No newer convenient
baseline, source sync, production optimization, observer subtraction or budget
revision was introduced. The previous worsened roadmap/native p95 and failed
memory results remain. Release/installation, live Home, historical unknown Execs,
copied identities and the immutable realistic fixture were untouched.

Review found that completeness was computed before resource evidence loaded;
it now runs after coverage evaluation. Regression cases retain missing tables,
missing resources, unsuccessful recording and gaps at either end. Release's
immediate goal and current schedule/publication/installation memory were inspected;
older incidents were not reread and provide no Desktop performance proof.
Check: focused recorder/runner pytest PASS 60; Ruff/diff PASS; two bounded actual
record/export probes PASS; full-hour acceptance remains unavailable.

## Capture attribution and missing-checkout reads — October 6

Jack Heart requested a repair of the original four-round memory failure, preserving
all bitmap/input endpoints and the <32 MiB budget. The original observer at
`cc0d22bc90b9611da569a5048a7ac308a0bcad5e` reproduced **+51.6 MiB**, with 20/20
endpoints and five preserved rounds over 150 seconds. Private receipts remain at
`/tmp/loo304-memory-current-20261006/`. Its first native capture moved RSS from
240.4 to 291.6 MiB before OCR, then 324.0 MiB after OCR. An intrusive process map
found 35.2 MiB live default-zone allocations, 89.9 MiB resident in that zone,
35.9 MiB empty large allocations and 17.2 MiB resident IOSurfaces. These categories
are not additive allocation ownership or proof that every retained byte belongs
to capture.

A separate fresh-Home diagnostic retained all five native launches, input replies,
focus, navigation, drafts, selection, pane layout and history checks, but omitted
native bitmap/OCR. Four-round growth was **+7.8 MiB**; all fifteen non-native
capture endpoints remained. `/tmp/loo304-native-no-capture-20261006/` and the exact
experimental source in `/tmp/loo304-handler-memory-20261006/` retain the proof.
Both diagnostic runs included a process map. This supports capture-dependent
allocation pressure in this bounded workflow, not a general absence-of-leaks
claim. The omitted observation makes the diagnostic **ineligible for acceptance**;
no observer cost is subtracted from the original failure.

### Rejected observer changes

Fixed-image probes retained the identical image and both fast/accurate recognizers.
All completed observations returned the same text hash. Reusing one image handler
for both passes exceeded the 512 MiB guard at observation 18 in both trials;
the original completed 40/40 twice. The background-processing hint produced no
useful reduction. CPU-only processing completed 40/40 and reduced fixed-image
residency, including through the current compute-device API, but the fully restored
mounted workflow still failed at **+51.2 MiB** despite 20/20 endpoints and five
preserved rounds (`/tmp/loo304-native-cpu-ocr-20261006/`). All experiments were
removed. Bitmap capture, recognizers, deadlines and measurement endpoints remain
unchanged. Earlier buffer-reuse/autorelease failures remain rejected too.

The next memory investigation belongs at the AppKit capture allocation boundary:
compare equivalent-pixel capture implementations or obtain allocation stacks for
that phase. These observations support no production cache purge, renderer patch
or budget revision. They do not establish compositor paint or one-frame input.

### Supported read repair

Investigation continued with the measured roadmap cost. Sharing repository Team
configuration across Waves did not reduce the 67 Git processes or improve latency;
that patch was rejected. Its 84 successful reads, worsened tail and complete row
preservation remain in `/tmp/loo304-portfolio-read-20261006/`.

Trace receipts instead showed **31 of 49 common-directory Git launches targeting
absent historical checkouts**. `git_common_dir` now sets `Command.current_dir(repo)`
instead of passing `git -C repo`. The OS reports missing cwd before Git executes;
existing repositories still use Git's resolver. No filesystem resolver, existence
cache, configuration bypass or authority change was added. The regression covers
nested directories, linked worktrees, symlinks, removal and recreation at the same
path; the existing alias/fresh-read regression also passes.

Normal release builds completed before the alternating pair on fresh isolated
copies of the unchanged 39-table snapshot. The retained pre-change binary has the
same Rust sources as `cc0d22bc9`; the candidate adds only the working-directory
change. [Exact hashes, samples and preservation](capture-cwd-summary.json) accompany
private receipts at `/tmp/loo304-cwd-read-20261006/`.

| Direct CLI, twenty warm successes each | Before median / p95 | After median / p95 |
| --- | ---: | ---: |
| Roadmap wall time, ms | 682 / 1761 | 548 / 764 |
| Roadmap child CPU, ms | 643 / 768 | 529 / 624 |
| Git launches per roadmap | 67 / 67 | 36 / 36 |
| Common-directory launches per roadmap | 49 / 49 | 18 / 18 |
| Session-list wall time, ms | 363 / 414 | 389 / 557 |

All 84 reads passed. Roadmap load medians matched at 22.4 (maximum 33.2), while
Session load medians were 31.5; host activity remained variable. Session timing
worsened and both paths still miss 300 ms. First reads remain separate in the JSON.
All retained rows, identities and storage types across 39 tables matched; each
copy appended 42 inspection Execs. Responses differed only in fresh timestamps and
525 verified age comparisons. Snapshot/config hashes stayed unchanged. Fewer Git
launches establish a targeted read reduction, not overall idle Desktop quietness.

### Unchanged-observer four-round comparison

Both normal mounted runs passed **20/20 endpoints and five preserved rounds** over
150 seconds, using identical Swift test-binary and measurement-source hashes.
No process-map inspection, builds or row audit overlapped this pair. Both memory
results **failed**: **+43.8 → +41.1 MiB** after four full rounds. That difference
does not establish a memory repair. Native reopening warm medians worsened
**926 → 1184 ms**; each scenario has only four warm samples, so no p95 is reported.
The accepted comparison contract remained available, and source/CLI/config hashes
were stable. One visible window, focus, input, history/native identity, drafts,
selection and pane layout survived; key-window identity stayed unavailable (-1).

Each side retained 150 CPU samples. App CPU median/p95/max was
0.9/11.8/55.3% → 1.0/15.3/72.7%, excluding children. CLI starts/ends were
258/258 → 252/252; SQLite statements 120,020 → 118,538 and emitted rows
1,446,699 → 1,428,896. Eleven unavailable comment reads per side remain visible.
These sequential short runs and variable load establish no overall CPU, native
latency or memory improvement. Bitmap/OCR still is not compositor paint.

Review rejected the ineffective configuration cache and every unproved observer
repair. The installed Home, historical unknown Execs, original snapshot, retained
traces and release controller were untouched. Checks: resolver and existing
alias/fresh-read regressions PASS; formatting/all-target Clippy/release build PASS;
84/84 matched reads and 39-table preservation PASS; mounted 20/20 each PASS
preservation / FAIL memory. Recorder readiness, faithful full-interval tracing,
compositor/input proof, independent Swift attribution and both original full-hour
variants remain open. The next measured read costs are the remaining existing-
checkout Git resolution and Task-work reads; preserve fresh configuration and
exact execution authority when investigating them. Publication only.

## Capture allocation stacks — October 6

Jack Heart requested bounded attribution of the remaining +43.8/+41.1 MiB
matched growth failure, without revising observation or acceptance. Two contained
`terminalMemoryPhases` probes completed with the retained test executable and
owned cat PTY. No production or ordinary observer code changed. The
[capture stack summary](capture-stacks-summary.json) records executable/receipt
hashes, exact private paths and selected stacks. The retained executable was not
rebuilt; checkout HEAD identifies the investigation, not authenticated build provenance.

The first probe recorded RSS 151.7→229.1 MiB at terminal bitmap capture, then
293.9 MiB after OCR and 368.7 MiB after recapture. Stack logging and external
inspection are intrusive; this diagnostic deliberately retains the first bitmap.
These figures neither replace the matched failure nor measure a production leak.

`malloc_history -allBySize` found the same three Ghostty `renderer.Metal.initTarget`
regions (54,067,200 bytes total) before capture, after capture and after recapture.
The retained bitmap's `CGBitmapAllocateData` allocation was 17,924,096 bytes.
The second probe's VM allocation stacks additionally identify two capture-created
allocator regions: 18,038,784 bytes beneath QuartzCore `copy_image_block_set`, and
17,956,864 bytes beneath CoreGraphics `create_image_data_handle`. Both paths run
through AppKit `cacheDisplayInRect`, layer rendering and `CGContextDrawImage`.
This attributes capture-created VM storage, extending the earlier anonymous maps;
it does not equate mapped bytes with live objects or resident bytes.

The no-compaction/full-events attempt returned 55,961 ALLOC and 208 VM_ALLOC
records, **no free events**, with stack-logging-lite frames. It therefore failed
to establish allocation/free lifetimes. Do not interpret absent free records as
a leak. The live size summaries retain the bitmap but do not list those two
large capture allocations; this is consistent with allocator-retained storage,
not proof of when or how every byte was released. The two runs took about twenty
seconds each; retained receipts total under 600 MiB. No full-hour trace ran.

No supported production repair follows: the existing render targets did not grow,
and deleting bitmap observation or purging memory would not meet the contract.
Next useful check: capture-stack attribution in the realistic workspace, or a
same-pixel capture implementation comparison targeting those CoreGraphics copies.
Any candidate must preserve full resolution, both recognizers, identity/input
checks and deadlines, then pass the original matched workload; no observer
subtraction or warm-up change is accepted. Latency, growth, independent Swift,
compositor/input, trace coverage/capacity and both full hours remain open.

Check: both contained phase probes PASS; free-history attribution INCOMPLETE;
receipt hash/summary verification PASS. No implementation change, matched rerun,
publication, merge or Task completion.

### Direct bitmap drawing rejected — October 6

Jack Heart requested a bounded same-pixel comparison before another matched
workload. From checkpoint `923339b70`, a temporary probe replaced only the capture
call with `NSView.displayIgnoringOpacity(_:in:)`, drawing into the same AppKit
bitmap representation with its full backing-scale transform. The existing
contained `terminalMemoryPhases` setup, owned cat PTY, input and both unchanged
recognizers remained. This tests bypassing the `cacheDisplay` wrapper; it does
not substitute a layer snapshot into acceptance. The original capture ran
immediately before and after the candidate, without intervening terminal input.

All three images were **2800×1600**. The two original RGBA buffers were exactly
identical. The candidate differed at **4,642 of 4,480,000 pixels** and omitted
terminal glyphs: both recognizers returned empty results, while each original
capture returned the same two rows per recognizer. The fast recognizer retained
the exact `owned-terminal-memory` input marker; the accurate recognizer's
imperfect transcription was identical across the original captures. Candidate
image inspection confirmed missing text, not a mere OCR mismatch. Full resolution
and a mostly matching background are insufficient evidence of native fidelity.

**Candidate rejected before memory comparison or full workload.** Single capture
times were 39.3/40.8/37.1 ms (original/direct/original); sequential RSS includes
previous captures and OCR and supplies no memory-saving claim. The harness passed
execution in 10.7 seconds, but the pixel/semantic comparison failed. No p95,
production repair, observer replacement or acceptance follows. The temporary
Swift edit was preserved privately, then restored byte-for-byte; ordinary
capture, warmup, recognizers, deadlines and all original workloads are unchanged.

The existing [stack summary](capture-stacks-summary.json) now retains comparison
counts, events, source/probe/build/executable hashes and private receipts under
`/tmp/loo304-capture-compare-20261006/`. Original allocation and failure evidence
remains intact. Review rejected treating the successful test-process exit or
99.9% matching background as equivalence. A subsequent alternative needs faithful
native/composited glyph capture first; repeating this direct-drawing path cannot
supply it. Realistic-workspace allocation attribution remains a useful independent
check. Latency, four-round <32 MiB growth, independent Swift attribution,
compositor/input proof, recorder readiness/capacity and both full hours remain open.
No installed Home, historical Exec, release or publication operation occurred.

Check: fresh Swift test build PASS; contained probe execution PASS; full-resolution
pixel/OCR equivalence FAIL (candidate discarded); source restoration/diff PASS.

### Capture object lifetime — October 6

Jack Heart requested checking natural autorelease lifetime before proposing another
pool repair. The retained `/tmp/loo304-capture-pool-20261006/experiment.swift` already
enclosed the entire synchronous capture and both recognizers. Its original
20/20 endpoints and **+53.0 MiB** four-round failure remain the workflow evidence;
that rejected experiment was not repeated as a new optimization.

A fresh build from `d686b1a88` plus temporary weak-reference instrumentation ran
four contained cat-PTY processes in unpooled/pooled/pooled/unpooled order, eight
full captures each. Weak references observed NSBitmapImageRep, Vision requests,
handlers and recognition results immediately after return and after the ordinary
5 ms suspension. All 32 captures retained the owned terminal marker.

| Mode/order | Bitmap alive at return / after suspension | First→last post-suspension RSS |
| --- | --- | --- |
| Unpooled, 1 | 1 / 0 at every capture | 203.7→369.4 MiB |
| Pooled, 2 | 0 / 0 at every capture | 209.4→376.9 MiB |
| Pooled, 3 | 0 / 0 at every capture | 205.8→370.7 MiB |
| Unpooled, 4 | 1 / 0 at every capture | 205.1→370.0 MiB |

Handlers and recognition results were gone at every return. Two requests remained
in each process, without increasing across captures. The explicit pool shortens
bitmap lifetime until return; ordinary suspension already ends that lifetime.
Persistent RSS growth therefore cannot be explained by accumulation of these
tracked objects across the observed iterations. Internal CoreGraphics allocations,
framework caches and allocator page retention are not measured by these weak
references; their ownership/lifetimes remain unresolved.

Both recognizers and the full 2800×1600 capture stayed unchanged. Every process
produced the same set of two byte-identical PNG states, with identical recognized
text for matching states; temporal sequences differed. This is lifetime diagnosis,
not exact repeated-workflow equivalence or a latency/memory acceptance run. PNG
serialization and instrumentation add costs. No diagnostic RSS is subtracted from
the original failed growth, and no deadlines, warmup or capture workload changed
in the ordinary runner. The original Swift source was restored byte-for-byte.

Private source, scripts, 32 images and journals remain under
`/tmp/loo304-object-lifetime-20261006/`; `capture-stacks-summary.json` records hashes
and per-capture counts. Review rejected a pool patch despite its earlier release
point: neither this comparison nor the prior full workflow demonstrates a retained
memory benefit. Next useful evidence is internal capture-storage lifetime in the
realistic workspace, independent of wrapper lifetime. All original acceptance
obligations remain. About 13 GiB free still cannot support the measured full-hour
trace, and recorder readiness/coverage remains unresolved; no full hour was run.

## Repeated Task Session membership — October 6

Jack Heart requested continuing measured CLI/native latency work from `6842816c3`,
with publication to PR #1460 and no merge or Task completion. Checkpoint `b1a75d2ff`
preserves the preceding uncommitted lifetime diagnosis. Exact distributions,
preservation and receipt hashes are in [task-membership-summary.json](task-membership-summary.json);
private scripts and original outputs remain at `/tmp/loo304-next-read-20261006/`.

Fresh native sampling produced no usable stacks: the first launch used a
noncanonical CLI path and was refused before execution; corrected launches exited
successfully but sampling either failed attachment or returned an empty call graph.
These are failed profiling attempts, not new attribution. Only disposable database
copies created for these attempts were removed, with source/copy hashes and their
added inspection Execs retained. Existing snapshots, traces and historical records
were untouched.

Bundled SQLite **3.53.2** isolated repeated Task Session selection in `exec_ids`:
Session history and current driver arms independently evaluated the same membership
expression. One query-local materialized CTE now supplies both arms. The expression,
checkout boundaries, explicit scopes, bindings, Flow associations, unfinished filter
and ordering are unchanged. There is no cache across queries, schema change,
Git/config shortcut or authority relaxation.

The original and candidate ID selections matched for **all 342 Tasks** (598 selected
IDs across Tasks, before the outer unfinished filter). VM steps fell
**30,854,354 → 17,549,229**. Three paired bundled-engine runs measured CPU seconds
1.186/0.725, 0.967/0.579 and 0.945/0.577. This isolates repeated SQL work, not the
whole Task reader or rendered latency. Existing membership/alias/scope/history
regressions passed, as did exact historical-uncertainty execution protection.

The retained baseline binary contains Rust through `4151c79a6`; inspection found
no later Rust changes through `b1a75d2ff`. The candidate is a normal release build
with only this SQL change. Builds and checks finished before alternating reads on
fresh copies of the same immutable snapshot and current authored configuration.

| Direct reads, twenty warm successes each | Baseline median / p95 | Candidate median / p95 |
| --- | ---: | ---: |
| Roadmap wall time, ms | 994 / 2123 | 939 / 1346 |
| Roadmap child CPU, ms | 805 / 883 | 786 / 907 |
| Session-list wall time, ms | 488 / 1480 | 460 / 1854 |
| Session-list child CPU, ms | 405 / 472 | 410 / 456 |

All **84 reads** passed. Git counts remained 36 per roadmap and three per Session
list. Roadmap load medians matched at 36.7 (maximum 42.5); Session load medians
matched at 64.3 (maximum 68.0). High, variable contention limits timing attribution;
Session p95 and roadmap CPU p95 worsened. Neither path meets 300 ms. All retained
rows/types/identities across **39 tables** matched, including unknown historical
Execs; each copy added only 42 inspection Execs. Responses differed only in fresh
timestamps and 525 verified age comparisons. Snapshot and config hashes match.

The unchanged mounted observer first passed baseline **20/20** endpoints and five
preserved rounds over 150 seconds. The first candidate failed its first warm
native endpoint: **16 successes, one timeout, three unstarted**. Focus/surface/history
were ready at roughly 1.64 seconds, then a 3.42-second OCR call consumed the remaining
five-second deadline; the failed endpoint ended at 5.64 seconds. This establishes
the observed deadline sequence, not why OCR stalled or a product allocation defect.
Its incomplete recorder (38 samples, 113/112 CLI starts/ends), failed journal and
unavailable comparison remain retained. No timeout or observer change followed.

After observed host load fell to about 14, one explicitly separate candidate
confirmation passed **20/20** endpoints and five preserved rounds. It uses the same
Swift binary, measurement-source hash, sandbox, fixture and observer as the baseline
and failed candidate. Baseline/confirmation source/CLI/config stability passed.
Unequal load and the retained intervening failure preclude a general speedup claim.

- Fourth-round RSS growth **failed both: +42.6 / +44.5 MiB**. The failed candidate
  never reached round four; its missing value is not zero.
- Native warm median: 1161 / 1091 ms. Warm Task: 366 / 404 ms; Task reopen:
  385 / 400 ms. Four warm observations provide **no p95**.
- App CPU median/p95/max: 0.9/16.0/92.3% / 0.9/20.0/68.8%, 150 samples each,
  excluding children. CLI starts/ends: 254/254 / 252/252. SQLite statements:
  119,036 / 118,536; emitted rows: 1,434,815 / 1,428,872.
- Both complete runs preserved native identity/history/input, drafts, selection,
  layout and focus with one visible window. Eleven unavailable reads per run remain
  explicit. Bitmap/OCR is not compositor paint; hitch/hang and full-interval trace
  coverage remain unmeasured in this diagnostic mode.

Review retained the query-local reduction and rejected interpreting it as Desktop
acceptance. Existing-checkout Git resolution and remaining Task-work costs still
need attribution; empty native samples cannot select the next repair. Original
latency/read budgets, failed <32 MiB growth, independent Swift attribution,
compositor/input, recorder readiness/capacity and both full hours remain open.
About 10 GiB available storage does not support the retained full-trace rates.
Release's immediate goal/current schedule and publication/installation memory were
read; older incidents were not reread and provide no Desktop acceptance.
Checks: release build, fmt, all-target Clippy PASS; Task-work 7 and historical
uncertainty 1 PASS; direct reads 84/84 and 39-table preservation PASS; mounted
baseline/confirmation 20/20 each PASS preservation / FAIL memory, first candidate
FAIL native deadline. Publication only.

## Git attribution and fixture read boundary — October 6

Jack Heart requested attribution before another resolver repair. The retained
release executable from `5f696f158` was reused without rebuilding. Three bounded
roadmap reads and three Session reads used the original sandbox, isolated fixture
copy and instrumentation. [Receipts](git-attribution-summary.json) retain hashes,
all command outcomes, timings and 39-table preservation; raw outputs/scripts stay
at `/tmp/loo304-git-attribution-20261006/`.

Each roadmap launches:

| Git operation | Count | Successful |
| --- | ---: | ---: |
| Common directory discovery | 18 | 2 |
| Checkout status | 12 | 0 |
| Top-level discovery | 2 | 2 |
| Exclude-path lookup | 1 | 1 |
| Branch-tip lookup | 3 | 0 |

Twelve common-directory calls come from distinct existing Task checkouts:
`task_configuration_refusal` resolves the configured agent and calls
`checkout_execution_boundary`. The following status and branch-tip reads provide
separate work/delivery evidence. They cannot be deleted as duplicate lookups.
The six remaining common-directory launches cover startup/current-repository and
Wave repository resolution. Empty native sample stacks remain unusable evidence.

All 12 sibling `.git` reads explicitly report `Operation not permitted`. The
sandbox permits the primary repository and exact authored Wave configs/goals,
but not those sibling markers or the other repositories' Git metadata. Git exit
128 becomes unavailable action evidence while the outer roadmap still exits zero.
This narrows the earlier performance claims; unchanged responses and retained
SQLite rows do not establish usable repository observations.

A separate diagnostic added **read-only Git metadata access** for those exact
12 checkouts, retaining all write, network, credential-home and process restrictions.
The same discovery commands then passed 36/36, versus 0/36 under the original
policy, in three paired repetitions. It granted no checkout-content reads and
ran no CLI, provider, status, mutation or mounted journey. Total time including
one sandbox launch per command was 545 ms refused / 601 ms readable. That overhead
is different from Git children of one CLI; it is not a production speed comparison.
The private policy is diagnostic only; the ordinary runner remains unchanged.

Original-policy roadmap wall times were 1869/477/472 ms; child CPU 553/448/451 ms.
Session reads were 254/235/236 ms. Git's internal Trace2 totals were 26.9/25.9/27.1 ms
per roadmap, including 14.0/13.1/13.9 ms common-directory work; these exclude process
startup and parent waiting, so they cannot establish Git's full cost. Three samples
supply no p95. Load was about 25.2. All six CLI starts/ends were retained; all 39
source tables remained identical, with six new inspection Execs on the copy only.

No production repair is selected. The next measurable fixture step is to provide
bounded read-only checkout and Git-metadata observations for both variants, retain
per-command exits, and prove that successful discovery/status and failed access are
distinguished. Hash observed metadata/configuration before and after and reject
changed-input comparisons; keep missing/recreated checkout and alias cases. Never
expand writes or copied execution authority. Re-establish the baseline under that
contract before claiming existing-checkout latency improvement. This diagnostic
advances representative-workload attribution, not Desktop acceptance.

Storage readback was 7.1 GiB free, below the recorder's 8 GiB admission threshold;
no trace was attempted. Original CLI/native budgets, failed <32 MiB growth,
independent Swift attribution, compositor/input and both hours remain open.
No mounted replay was run: there is no product or runner change to validate.
Review rejected replacing Git semantics or dropping boundary checks based on failed
reads. Publication of this evidence correction does not authorize merge/completion.


## Corrected repository observations — October 6

Jack Heart requested the demonstrated harness correction and a fresh matched
baseline before another production optimization. [Summary](corrected-git-summary.json)
retains command outcomes, distributions, binary/input hashes and preservation.
Raw scripts and receipts are `/tmp/loo304-corrected-git-20261006/`.

The runner now grants read-only access to exact snapshot-named Git checkout trees,
plus their common/per-worktree Git metadata. Git status needs tracked content and
per-directory ignore files, beyond the prior twelve `.git` markers. No parent
source directory, credential Home, write, network, process or signal grant was
added. Copied clients still cannot execute. Exact-file policy experiments hit
macOS compiler limits; their failures remain in the raw receipts. The final
policy uses the existing bounded checkout-tree form. Symlinks cannot grant reads
outside those trees. Setup enumerates Git files inside the same sandbox.

Before/after manifests agree **across all four observations**: 62,120 file
fingerprints, 378 recorded checkout paths (34 existing), names, aliases, inode
identities, content/modes, Git metadata and authored configuration. Hashing occurs
outside measured commands and warms filesystem caches equally on both variants.
The input set is deliberately conservative, including reachable metadata files
that a particular command might not read. Removed/recreated checkout, alias,
changed-content and refused-access regressions pass. Raw after-manifests survive;
changed inputs or failed/incomplete Git observations reject report comparison.

The retained release binaries are the preceding Task-membership pair, with exact
hashes matching its provenance: baseline Rust `4151c79a6` (unchanged through
`b1a75d2ff`), candidate materializing Session membership within one query.
Neither was rebuilt or promoted. Both use the same corrected policy, immutable
prepared database, host and instrumentation. This is a renewed **CLI-only**
comparison, not independent Swift or rendered acceptance.

| Read, twenty warm observations each | Baseline median / p95 | Candidate median / p95 |
| --- | ---: | ---: |
| Roadmap | 1146 / 1189 ms | 1117 / 1164 ms |
| Sessions | 269 / 274 ms | 270 / 277 ms |
| Roadmap child CPU | 1207 / 1250 ms | 1192 / 1239 ms |
| Session child CPU | 264 / 269 ms | 266 / 270 ms |

All **84/84 outer CLI reads** exited zero, without timeouts. Each roadmap now
records **94 Git children: 93 successful, one failed**, including 25 successful
status commands. Each Session read records 3/3 successful Git children. The
remaining discovery failure is consistent with the existing historical `cadenza`
path no longer being a Git checkout; separate discovery probes retain its exact
cwd and outcome. These failed reads stay visible and prevent full report scoring.
The prior 31/36 failed-child cohorts remain limited evidence; they are not this
baseline. Per-command traces retain all 4,074 exits across the pair.

All 39 tables retain identical original rows; each copy gained only 42 inspection
Execs. All 42 response pairs agree apart from fresh timestamps and **1,554 verified
age differences**. Each variant records 42/42 CLI starts/ends, 525 connections,
94,628 SQLite statements and 1,194,461 rows. This proves no reduction in refresh
volume. Host load and first-observation timings remain in the summary. Roadmap
still misses 300 ms; the small query delta is not Desktop acceptance.

A separate one-second exact-process sample found 559/743 main-thread samples in
the completion gate's Exec read, including SQLite page reads; fresh Git boundary
checks account for only part of the remainder. No exact status command repeats
within the sampled roadmap. Forcing the existing unfinished index on the outer
Exec selection preserved all 342 Tasks' results (57 rows), but VM steps rose
17,559,770→17,922,832 and CPU stayed 1.079 seconds. That hypothesis was rejected;
no Rust patch or new index was adopted. The useful next query investigation is
the full completion-gate read and its page access under bundled SQLite, retaining
all membership/history and fresh launch checks. A partial sample cannot allocate
the entire command's CPU or justify deleting those checks.

The corrected harness passes 63 Python tests, a Swift test build and five contained
native proofs, including copied-client refusal, history and process restrictions.
No mounted workload or trace ran. Storage is 6.6 GiB, below the 8 GiB admission
threshold. All original latency, <32 MiB growth, independent Swift attribution,
compositor/input, recorder readiness/capacity and both full-hour obligations remain.
Release's immediate goal and current schedule/publication/installation memory were
read; older incidents were not reread and provide no Desktop acceptance.
Review retained the real failed Git child, rejected the unsupported SQL hint and
kept publication separate from merge/completion.

## Completion-gate history, concurrent Task details and command startup — October 6

Jack Heart requested the completion-gate repair with a matched proof on the
corrected Git policy. [Summary](gate-read-summary.json) retains every cohort,
hash and limit; raw receipts are `/tmp/loo304-gate-read-20261006/`.

**Fixture.** The prepared fixture `d9dc08b5…` and its retained snapshot no longer
exist. This pair is a new one, never compared with earlier cohorts. Both sides
are clones of the surviving `/tmp/loo304-release-20261006/index-diagnostic.db`:
the prepared fixture plus one diagnostic index. All 39 tables match the prepared
manifest's per-table hashes on both sides, and each schema equals a fresh Home
created by its own binary. The files are not byte-identical to the lost fixture.
Before is HEAD `d688c47fe`'s Rust, unchanged since `5f696f158`.

**Three causes, three repairs.**

- *Event history.* The gate's Exec read walked every event of a Task's Sessions
  and sought each table row to read `exec_id`: 521,637 events, 1,669 naming an
  Exec. A partial index on `(session_id, exec_id)` holds only those. Bundled
  SQLite 3.53.2 returns identical rows for all 342 Tasks. The earlier 559/743
  sample followed a fresh clone and measured cold page reads. With the index
  alone, the first read after cloning fell 2863→1216 ms and the warm median
  1101→1002 ms.
- *Serial Git children.* Warm, 550 of 629 main-thread samples waited on 90 Git
  children run one after another. Task details now gather on four workers and
  return in request order. Every observation still runs fresh; none is cached,
  skipped or reordered in the output. Eight workers saved another 30 ms of wall
  time for 280 ms more CPU and were rejected.
- *Command startup.* `lf home id` took 194 ms on an empty Home; `lf --version`
  takes 12. A draft-bearing build replayed every migration in every process
  to validate its schema. The build now embeds that reference as it already
  does for the canonical schema: 35 ms. An installed release carries no drafts
  and never paid this, so about 160 ms of each baseline read below is a cost
  of development builds, not of the installed product.

| Read, twenty warm observations each | Before median / p95 | After median / p95 |
| --- | ---: | ---: |
| Roadmap | 971 / 1008 ms | 412 / 428 ms |
| Sessions | 259 / 264 ms | 98 / 100 ms |
| Roadmap child CPU | 1008 / 1049 ms | 1067 / 1117 ms |
| Session child CPU | 254 / 260 ms | 94 / 96 ms |

All 84 reads exited zero on unchanged inputs: 57,887 file fingerprints and 378
recorded checkouts (30 existing) agree across all four manifests. Each roadmap ran 90 Git children on both sides, 89
successful; the historical `cadenza` path still fails discovery and still
prevents full report scoring. All 39 tables keep identical rows, each copy
gaining 42 inspection Execs. All 42 response pairs agree apart from fresh
timestamps and 1,554 verified ages. Statement counts are identical, so refresh
volume did not fall. Each measured read includes about 50 ms of `sandbox-exec`.
Sessions meets 300 ms. Roadmap does not, and its CPU rose 6% despite losing the
replay: overlap costs CPU.

Two earlier runs are **rejected**: another Task committed to the shared
repository mid-run and the manifests changed. Their numbers agree with the
table and stay in the summary beside the index-only and eight-worker cohorts.

**Mounted pair.** One sequential run each, 84/84 endpoints, twenty warm
observations per scenario, 21 preserved rounds, no trace.

| Warm scenario | Before median / p95 | After median / p95 |
| --- | ---: | ---: |
| Cold workspace | 870 / 896 ms | 597 / 673 ms |
| Native Session reopen | 794 / 927 ms | 531 / 643 ms |
| Reopen Task | 198 / 240 ms | 234 / 332 ms |
| Warm Task | 260 / 268 ms | 266 / 276 ms |

Reopen Task **worsened**; the cause is unknown. Four-round growth **failed** on
both sides: +53.4 and +51.1 MiB. Recorder-sampled RSS growth rose 47.7→96.6 MiB,
unexplained. Application CPU p95 was 23.2%→13.4%, excluding children. CLI
processes rose 547→564: faster reads admit more refreshes. The runner refuses
its own comparison because the fixture files differ; equality is the table
proof above. Bitmap and OCR are not compositor paint.

**Remaining.** Roadmap still spends most of its 412 ms on 23 fresh
`git status` reads, about 10 ms each alone and slower when overlapped. Going
lower means observing checkout changes instead of polling them, which belongs
with change-driven refresh. Every original obligation stays open: 100 ms
interactions, 300 ms roadmap, growth under 32 MiB, independent Swift
attribution, compositor and input proof, recorder readiness and both full
hours. No trace ran. Nothing touched the installed Home.
