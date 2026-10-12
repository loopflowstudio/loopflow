# Next CI measurement

Jack Heart's target remains required CI under five minutes across ten consecutive
real typical-code PR runs. PR #1543 passes but does not meet that target. Its
first build-reuse improvement awaits Jack's merge decision; no arming is implied.

## Measured bounds

Run 38314949621: Rust build 131s, setup approximately 17s, test execution 886.890s,
2,548 passing cases summing to 3,516.38 test-seconds. The longest watcher case
takes 156.008s. The lower bound with four test slots per shard is
`max(3516.38 / (4 * shards), 156.008)`, before compilation or transfer.

| Ordinary runners | Optimistic test floor | Build + setup + test floor |
| --- | ---: | ---: |
| 2 | 439.5s | 587.5s |
| 4 | 219.8s | 367.8s |
| 6 | 156.0s | 304.0s |
| 8 | 156.0s | 304.0s |

This is a bound, not a prediction. Hash partition imbalance, discovery,
artifact transfers and runner queues add time. CPU contention may change case
durations. Adding shards alone cannot be claimed to meet the target.

## Next candidates to implement and measure

1. Move the existing planning-reconnect integration target to the existing CLI
   smoke job, preserving both Flow and watcher cases and external-network
   containment. That job already compiles the CLI and took 89s. Verify marginal
   integration compilation, then measure its complete path; do not count the
   89s + 156s estimate as observed acceptance. The remaining archive can omit
   this target so each case has one CI execution owner.
2. Compile remaining Rust cases once, archive, and partition execution on
   ordinary runners. Prioritize the longest tests so they do not start at the
   tail. With the watcher target separately owned, the longest remaining case
   is 120.359s: eight perfectly balanced shards have a 268.4s build/setup/test
   floor, leaving only about 32s for transfer, discovery and scheduling. Measure
   before choosing a permanent shard count; include total runner occupancy.
3. Reuse disposable installation dependency compilation across CI runs. Current
   managed compilation costs 187s and the distinct published CLI costs 69s;
   total job is 328s. Preserve disposable OS-account authority, exact source,
   materialized migration inputs and separate published provenance. Cache
   misses must rebuild normally, never accept a stale executable. Retain the
   currently measured cold path and report cold/warm costs separately.

No paid hardware, shorter behavioral observation windows, deleted assertions,
or new production interfaces are selected by this plan. The independent test
code cleanup remains Task lf-6d3ff35. These implementation candidates need their
own delivery ownership after the first build-reuse PR; no competing worker has
been launched.

Local archive evidence: 392 MiB, 0.75s packaging, 3.24s extraction, one isolated
Flow proof passes from the extracted CLI. Linux transfer and cross-machine path
compatibility remain unproved. Nextest supports [archive reuse](https://nexte.st/docs/ci-features/archiving/)
and [test priorities](https://nexte.st/docs/configuration/test-priorities/).
