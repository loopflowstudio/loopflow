# Matched Rust read-path comparison — October 6

Both variants passed **84/84 endpoints**, with one first interaction and twenty
successful warm observations for each of four scenarios. Rust Session-list and
roadmap reads improved. Neither variant meets the memory budget; native reopening
p95 worsened. This is partial performance evidence, not Task acceptance.

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
