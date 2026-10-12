# Next CI measurement

Jack Heart requested a historical timing audit: last full code CI under five
minutes, largest sustained increases (with responsible PRs where traceable), and
test-count growth versus compilation, per-test cost and workflow changes.
An independent read-only contribution is researching this into
`docs/reviews/ci-timing-history.md`; it does not own implementation.

The historical audit is complete. September 30's last verified full PR ran in
4:36; later short PRs were checkpoint deferrals. Recent test-count growth is
31%, versus 6.6x execution time. Prioritize measured fixture and repeated-work
reductions before selecting the proposed shard count below.

Matrix fixture reuse retains all 70 independent cases: local execution fell
53.62s to 13.22s by restoring a closed, fully seeded Home/repository before
each command. Final three-test module pass: 12.64s. Hosted benefit remains
unmeasured; the existing broad error classifier is still a coverage limitation.

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

## Installation cache experiment

The local `--cache-dir` prototype binds only Cargo target and registry directories;
source and installation authority remain disposable. Six existing harness tests
pass. It is not wired into CI or published.

The empty Mac-host bind directory compiled in 166s, then unexpectedly rebuilt for
124s before the first migration proof passed (2.32s). The next proof began another
rebuild; the experiment was deliberately stopped, not passed. An unchanged tiny
Cargo project also rebuilt immediately on this bind mount, while the same project
on Docker's native filesystem reported Fresh on its immediate second build.
This implicates bind-mount behavior but does not establish the exact invalidation
cause or Linux behavior. Do not use these timings to predict GitHub cache savings.
Next validation needs Linux-native cache storage and separate cold/warm measurements.
Raw evidence: `/tmp/loopflow-installation-cache-cold.log`,
`/tmp/loopflow-bind-fingerprint*.log`, `/tmp/loopflow-native-fingerprint.log`.
