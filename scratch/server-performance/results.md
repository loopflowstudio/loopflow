# Desktop server boundary measurements — November 4, 2026

Jack Heart requested deletion of the embedded Loopflow library and a performance comparison.

Debug builds on the same Apple Silicon macOS host; one isolated local Task and one prepared Session. Each transport ran 31 sequential samples. Sample 1 is cold, the remaining 30 produce the warm median and nearest-rank p95 below. The final server run had no other tests/builds launched by this operator. Other work on the host was not controlled.

| Operation | Embedded median / p95 (ms) | Server median / p95 (ms) |
| --- | ---: | ---: |
| read_ms | 21.06 / 23.23 | 26.79 / 36.82 |
| write_ms | 50.68 / 54.47 | 87.60 / 123.74 |
| subscribe_first_ms | 26.95 / 40.03 | 33.07 / 43.04 |
| checkpoint_ms | 16.61 / 19.48 | 22.00 / 28.63 |

The read is the all-Task roadmap; the write renames the same Session; subscriptions wait for the first roadmap frame; checkpoints measure a response on an open subscription. They do not measure compositor presentation or arbitrary dataset scaling.

Server readiness: 257.43 ms. Stop/start plus a successful read through the same client: 283.45 ms. These are single observations, not distributions or full-app startup timings.

With one idle roadmap subscription, a five-second sample measured 0.19% of one CPU and 246.7 MiB RSS. No matched embedded idle-RSS sample was retained; this is the server footprint, not a memory-savings claim.

The SwiftPM debug Desktop executable shrank from 282,749,312 bytes to 57519360 bytes. The server CLI remains a separate executable. `nm -gU` finds none of the removed lf_read/lf_write/lf_subscribe/lf_pull/lf_string_free ABI symbols in Desktop.

Writes are slower because the server runs the full CLI, retaining routing, defaults and attribution. Reads and checkpoints also have a measurable overhead. This cutover does not claim a latency improvement. No caching or duplicated mutation executor was added to hide the cost.

Reproduce with inherited LF_* removed: `LOOPFLOW_BOUNDARY_PERF=/tmp/server.json swift test --package-path swift --filter ServerPerformanceTests`. The fixture uses the source-built target/debug/lf and a disposable Machine; no installed service registration. `baseline-harness.swift` preserves the pre-deletion harness; its implementation runs against the FFI at parent 8d6c7ee75. Raw samples and baseline artifact sizes are alongside this note.

The first attempt used Portfolio add, which scanned the host source directories and was discarded; Session rename replaced it in both compared runs. An inherited capture variable then rejected the initial private-Machine write; the successful baseline ran with LF_* cleared. An overlapping-test server run was discarded in favor of the isolated run.
