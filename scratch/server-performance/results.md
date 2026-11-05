# Direct server write measurements — November 4, 2026

Jack Heart requested removing per-write CLI startup while retaining telemetry,
then measuring the result and exercising the real Desktop app. Ordinary writes
now run the same mutation implementation as the CLI inside the server, with
request-scoped diagnostics in the normal `logs/lf.log` and no fabricated Process.

## Matched workload

Both runs use the current Swift `ServerPerformanceTests` harness, the same host,
a private Machine, one local Task and one prepared Session. Each contains 31
samples; discard the first and compute the warm median and nearest-rank p95
from the remaining 30. Subprocess uses the retained signed helper from
`/Users/jack/.lf/demos/loo593-server-5r7obf0c/Loopflow Server Live.app`;
direct uses the source-built `target/debug/lf` at `59287ba3e` plus the direct-write
working diff. Both are debug builds. Other host work, including the resumed Task
Graph, was not controlled; this is a local comparison, not a release benchmark.

| Operation | Subprocess median / p95 (ms) | Direct median / p95 (ms) |
| --- | ---: | ---: |
| Session rename | 87.38 / 105.30 | 10.77 / 20.63 |
| Roadmap read | 26.94 / 34.42 | 28.00 / 52.67 |
| Subscribe to first frame | 31.64 / 46.63 | 33.40 / 47.55 |
| Subscription checkpoint | 21.84 / 31.32 | 21.66 / 28.49 |

The rename median fell 87.7%, or about 8.1×. This measures the client/server
request round trip for one repeated rename, not all writes or UI presentation.
Read medians and checkpoints stayed close; read tail latency was worse. There
is no evidence here of a general read or startup improvement.

Single readiness observations: 256.72 → 269.88 ms. Stop/start and read through
the same client: 277.57 → 458.41 ms. Those are single observations, not latency
distributions; the slower restart remains visible rather than being discarded.
During a five-second idle subscription sample, both reported 0% CPU at the
measurement resolution. Server RSS was 244.5 → 255.5 MiB. There is no memory
savings claim. Raw samples: `subprocess-matched.json`, `direct.json`.

Reproduce with inherited `LF_*` unset, `LOOPFLOW_TEST_LF` pointing to the selected
helper and `LOOPFLOW_BOUNDARY_PERF` naming the output JSON:
`swift test --skip-build --package-path swift --filter ServerPerformanceTests`.
Build the current Swift harness first. The earlier embedded result below is
historical; only the two server variants above were rerun together.

## Real Desktop proof

See [direct-write-demo.md](../direct-write-demo.md): Task creation, conversation,
Session rename, file save and description save were exercised through the native
app. The ordinary edit traces share the real server PID and have distinct trace
IDs in `logs/lf.log`. Their individual durations are not substituted for the
benchmark distribution. A completed autonomous research Graph is not proved;
the private demo Machine lacked a managed Codex account.

The comprehensive served-read/write parity test passed on the measured code,
including errors, concurrent requests, large saves, default telemetry and
completion after viewer disconnect. Remote ownership routing also passed.
Later provider-admission cwd repairs and final Graph checks have their own
verification; they are not silently included in these earlier observations.

# Historical embedded-to-subprocess measurements — November 4, 2026

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

In this superseded implementation, writes were slower because the server ran the full CLI, retaining routing, defaults and attribution. Reads and checkpoints also have a measurable overhead. This cutover does not claim a latency improvement. No caching or duplicated mutation executor was added to hide the cost.

Reproduce with inherited LF_* removed: `LOOPFLOW_BOUNDARY_PERF=/tmp/server.json swift test --package-path swift --filter ServerPerformanceTests`. The fixture uses the source-built target/debug/lf and a disposable Machine; no installed service registration. `baseline-harness.swift` preserves the pre-deletion harness; its implementation runs against the FFI at parent 8d6c7ee75. Raw samples and baseline artifact sizes are alongside this note.

The first attempt used Portfolio add, which scanned the host source directories and was discarded; Session rename replaced it in both compared runs. An inherited capture variable then rejected the initial private-Machine write; the successful baseline ran with LF_* cleared. An overlapping-test server run was discarded in favor of the isolated run.
