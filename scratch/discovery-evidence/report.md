# Active Run discovery cost proof

All three populations and 60/60 warm observations pass. Publication during a long scan returns scanning before converging; cancellation interrupts traversal. The separately retained debug test executable and source hashes are in [the command receipt](measurement-command.json).

| Old Runs | One-shot reader ms | Cold retained reader ms | Warm p50 / p95 ms | Recovery ms | Resume visible ms |
|---:|---:|---:|---:|---:|---:|
| 100 | 371.8 | 365.7 | 179.3 / 182.8 | 367.1 | 375.7 |
| 10,000 | 1454.6 | 1417.6 | 178.9 / 200.1 | 1381.8 | 360.5 |
| 100,000 | 52297.6 | 50055.5 | 174.8 / 178.7 | 47580.5 | 366.4 |

Each unchanged warm observation visits **zero directories**, performs **four receipt reads** (one live native client and its ownerless capture, before/after projection), reads two live manifests, samples processes once, and retains two candidates. Dead native, capture and Exec receipts are not reread. Every resume attempt is retained; no resume attempt triggers a full rescan. Recovery explicitly revisits retained history.

Warm reader-process CPU medians are approximately 94 / 94 / 93 ms. Maximum observed warm resident memory is approximately 42.8 / 48.8 / 47.0 MiB. CPU excludes the ps child; RSS includes the test process and SQLite. Peak RSS in raw records is cumulative, including fixture construction. This is not an isolated per-phase memory peak.

At 100,000 old Runs, publication during the roughly 47-second recovery scan invalidates that observation and subsequently appears with the original clients. The cancellation receipt records one directory visited and approximately 169 ms total, establishing interruption after traversal begins. No provider is signaled by reader cancellation.

This compares cold and warm paths within one source-stable debug library executable on macOS 26.0.1 arm64. It does not compare a previous release, include CLI startup/serialization or measure UI readiness, configured-provider activity, compositor presentation or a product budget. The expensive cold scan remains visible; these results establish bounded warm discovery, not fast cold startup.

Raw results: [final log](cost-final.log), [all observations and summary](cost-summary.json). Earlier populations omit the ownerless capture and cannot be used as a matched latency baseline for this final population.
