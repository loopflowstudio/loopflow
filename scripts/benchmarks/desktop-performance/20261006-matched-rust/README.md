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

- Rust steady-state attribution now has a matched rendered pair. Release-profile
  timing is still missing. Next read profiling should separate remaining roadmap
  Git/status work, Task reads and refresh cadence; this debug pair does not prove
  the production latency ceiling.
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
