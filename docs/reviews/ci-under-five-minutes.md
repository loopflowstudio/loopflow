# CI timing baseline — 2026-10-12

Jack Heart requested required PR checks under five minutes, with merge-queue time reported separately. This baseline measures seven completed successful full PR/merge-group runs from the latest 15 runs. It excludes pushes, checkpoints, cancelled/failed runs and the running merge group. It is not the ten-run acceptance sample.

Source: GitHub Actions run and jobs APIs for `loopflowstudio/loopflow`, workflow `ci.yml`. Durations use job/step timestamps; workflow duration uses createdAt to updatedAt (completion proxy). Skipped jobs are excluded. Job start offset includes dependency and scheduler delay; it is not pure runner queue time. GitHub job created_at to started_at is available in the saved API evidence, but dependency release is the better scheduler boundary here. Push-to-created latency, merge queue admission wait, and push-to-merged are not yet measured.

## Full runs

| Run | Event | Workflow wall time |
|---|---|---|
| [38303294677](https://github.com/loopflowstudio/loopflow/actions/runs/38303294677) | pull_request | 20:56 |
| [38298671922](https://github.com/loopflowstudio/loopflow/actions/runs/38298671922) | pull_request | 20:44 |
| [38221505578](https://github.com/loopflowstudio/loopflow/actions/runs/38221505578) | merge_group | 20:17 |
| [38211831760](https://github.com/loopflowstudio/loopflow/actions/runs/38211831760) | merge_group | 18:40 |
| [38301001942](https://github.com/loopflowstudio/loopflow/actions/runs/38301001942) | merge_group | 20:26 |
| [38212094987](https://github.com/loopflowstudio/loopflow/actions/runs/38212094987) | merge_group | 19:46 |
| [38219397958](https://github.com/loopflowstudio/loopflow/actions/runs/38219397958) | pull_request | 20:56 |

## Job durations

| Job | Median | Range |
|---|---|---|
| rust-test | 20:09 | 18:22–20:40 |
| task-installation | 10:28 | 7:43–10:46 |
| swift-test | 6:38 | 5:46–7:32 |
| loopflow-ui-test | 3:30 | 2:44–3:59 |
| e2e-smoke | 1:24 | 1:03–1:32 |
| rust-lint | 1:15 | 0:54–1:16 |
| website-test | 0:55 | 0:52–0:58 |
| python-test | 0:34 | 0:31–0:37 |
| migration-check | 0:14 | 0:12–0:16 |
| architecture-check | 0:12 | 0:10–0:14 |
| scratch-clear | 0:06 | 0:05–0:08 |
| tests-result | 0:03 | 0:02–0:03 |

## Latest PR step breakdown (38303294677)

Workflow: 20:56. Rust starts at +0:13 and finishes at +20:51; aggregate finishes at +20:55. Scratch gate finishes at +0:10. Rust scheduling after that gate costs 3 seconds; Swift scheduling costs 57 seconds. These are observed dependency-release-to-start gaps, not direct capacity measurements.

### scratch-clear

Start offset: 0:02. Job duration: 0:08.

| Step | Duration |
|---|---|
| Set up job | 0:01 |
| Run actions/checkout@v7 | 0:04 |
| Select landing candidates and enforce queue scratch cleanup | 0:00 |
| Post Run actions/checkout@v7 | 0:00 |
| Complete job | 0:00 |

### rust-test

Start offset: 0:13. Job duration: 20:38.

| Step | Duration |
|---|---|
| Set up job | 0:02 |
| Run actions/checkout@v7 | 0:02 |
| Run dtolnay/rust-toolchain@stable | 0:01 |
| Run astral-sh/setup-uv@v7 | 0:02 |
| Materialize draft migrations for tests | 0:00 |
| Fingerprint workspace build profiles | 0:01 |
| Run Swatinem/rust-cache@v2 | 0:14 |
| Run taiki-e/install-action@nextest | 0:02 |
| Run Rust tests | 19:41 |
| Prove checkout refresh through the CLI | 0:28 |
| Post Run Swatinem/rust-cache@v2 | 0:01 |
| Post Run astral-sh/setup-uv@v7 | 0:00 |
| Post Run actions/checkout@v7 | 0:00 |
| Complete job | 0:00 |

### rust-lint

Start offset: 0:12. Job duration: 1:11.

| Step | Duration |
|---|---|
| Set up job | 0:01 |
| Run actions/checkout@v7 | 0:02 |
| Run dtolnay/rust-toolchain@stable | 0:01 |
| Run astral-sh/setup-uv@v7 | 0:01 |
| Fingerprint workspace build profiles | 0:01 |
| Run Swatinem/rust-cache@v2 | 0:05 |
| Run cargo fmt check | 0:05 |
| Run cargo clippy | 0:52 |
| Post Run Swatinem/rust-cache@v2 | 0:00 |
| Post Run astral-sh/setup-uv@v7 | 0:00 |
| Post Run actions/checkout@v7 | 0:00 |
| Complete job | 0:01 |

### python-test

Start offset: 0:12. Job duration: 0:32.

| Step | Duration |
|---|---|
| Set up job | 0:01 |
| Run actions/checkout@v7 | 0:01 |
| Run astral-sh/setup-uv@v7 | 0:03 |
| Run uv sync | 0:01 |
| Run Python tests without external networking | 0:24 |
| Post Run astral-sh/setup-uv@v7 | 0:00 |
| Post Run actions/checkout@v7 | 0:00 |
| Complete job | 0:00 |

### website-test

Start offset: 0:12. Job duration: 0:55.

| Step | Duration |
|---|---|
| Set up job | 0:02 |
| Run actions/checkout@v7 | 0:03 |
| Run astral-sh/setup-uv@v7 | 0:02 |
| Run website tests | 0:44 |
| Post Run astral-sh/setup-uv@v7 | 0:00 |
| Post Run actions/checkout@v7 | 0:00 |
| Complete job | 0:00 |

### task-installation

Start offset: 0:12. Job duration: 10:39.

| Step | Duration |
|---|---|
| Set up job | 0:02 |
| Run actions/checkout@v7 | 0:02 |
| Run astral-sh/setup-uv@v7 | 0:02 |
| Prove managed Tasks in a disposable installation | 9:30 |
| Prove native title hooks through published promotion | 1:00 |
| Post Run astral-sh/setup-uv@v7 | 0:00 |
| Post Run actions/checkout@v7 | 0:00 |
| Complete job | 0:00 |

### loopflow-ui-test

Start offset: 0:21. Job duration: 3:30.

| Step | Duration |
|---|---|
| Set up job | 0:02 |
| Run actions/checkout@v7 | 0:03 |
| Run dtolnay/rust-toolchain@stable | 0:09 |
| Run astral-sh/setup-uv@v7 | 0:02 |
| Fingerprint workspace build profiles | 0:01 |
| Run Swatinem/rust-cache@v2 | 0:20 |
| Install XcodeGen | 0:03 |
| Fingerprint Xcode build inputs | 0:05 |
| Restore Xcode build | 0:08 |
| Restore unchanged Xcode source timestamps | 0:01 |
| Generate Xcode project | 0:00 |
| Build Loopflow for testing (compile check) | 2:29 |
| Post Run Swatinem/rust-cache@v2 | 0:01 |
| Post Run astral-sh/setup-uv@v7 | 0:02 |
| Post Run actions/checkout@v7 | 0:00 |
| Complete job | 0:02 |

### architecture-check

Start offset: 0:12. Job duration: 0:10.

| Step | Duration |
|---|---|
| Set up job | 0:00 |
| Run actions/checkout@v7 | 0:02 |
| Run astral-sh/setup-uv@v7 | 0:02 |
| Verify the architecture map | 0:03 |
| Post Run astral-sh/setup-uv@v7 | 0:01 |
| Post Run actions/checkout@v7 | 0:00 |
| Complete job | 0:00 |

### e2e-smoke

Start offset: 0:13. Job duration: 1:25.

| Step | Duration |
|---|---|
| Set up job | 0:04 |
| Run actions/checkout@v7 | 0:02 |
| Run dtolnay/rust-toolchain@stable | 0:01 |
| Run astral-sh/setup-uv@v7 | 0:02 |
| Fingerprint workspace build profiles | 0:01 |
| Run Swatinem/rust-cache@v2 | 0:12 |
| Run E2E smoke test | 1:00 |
| Post Run Swatinem/rust-cache@v2 | 0:00 |
| Post Run astral-sh/setup-uv@v7 | 0:00 |
| Post Run actions/checkout@v7 | 0:00 |
| Complete job | 0:00 |

### migration-check

Start offset: 0:12. Job duration: 0:14.

| Step | Duration |
|---|---|
| Set up job | 0:01 |
| Run actions/checkout@v7 | 0:07 |
| Run astral-sh/setup-uv@v7 | 0:02 |
| Fetch canonical migration history | 0:00 |
| Verify migration namespaces | 0:01 |
| Post Run astral-sh/setup-uv@v7 | 0:00 |
| Post Run actions/checkout@v7 | 0:00 |
| Complete job | 0:00 |

### swift-test

Start offset: 1:07. Job duration: 7:20.

| Step | Duration |
|---|---|
| Set up job | 0:03 |
| Run actions/checkout@v7 | 0:10 |
| Run astral-sh/setup-uv@v7 | 0:03 |
| Run dtolnay/rust-toolchain@stable | 0:11 |
| Fingerprint workspace build profiles | 0:01 |
| Run Swatinem/rust-cache@v2 | 0:21 |
| Fingerprint Swift build inputs | 0:06 |
| Restore SwiftPM build | 0:26 |
| Restore unchanged Swift source timestamps | 0:01 |
| Build CLI for Swift transport tests | 1:47 |
| Test checkout observation's read-only sandbox | 0:03 |
| Test macOS installation process discovery | 2:36 |
| Build Desktop and run headless model and view tests | 1:23 |
| Check Swift multiplatform boundaries | 0:00 |
| Post Run Swatinem/rust-cache@v2 | 0:01 |
| Post Run astral-sh/setup-uv@v7 | 0:00 |
| Post Run actions/checkout@v7 | 0:00 |
| Complete job | 0:04 |

### tests-result

Start offset: 20:53. Job duration: 0:02.

| Step | Duration |
|---|---|
| Set up job | 0:00 |
| Aggregate test results | 0:00 |
| Complete job | 0:00 |

## Findings and proposed work

- Rust nextest: compilation 2:16; execution 17:23.868, 2,548 passed and 17 skipped. Total step 19:41.62. Existing suites already run in parallel. First investigate expensive fixture work and subprocess waits; then evaluate balanced nextest shards with one compilation. Compilation, artifact transfer and runner startup must fit the same five-minute budget. Sharding alone cannot remove the 2:16 compile floor.
- Slowest Rust cases: planning reconnect 156.229s; native runner quietness 134.280s; command/environment resolution matrix 121.612s; four Flow tests 61–79s. These are investigation targets, not evidence that their coverage should be deleted.
- Disposable installation: 9:30 managed-Task proof plus 1:00 native-title proof. Logs show repeated compilation inside the proof. Evaluate sharing compiled artifacts and separating independent scenarios while preserving disposable OS-account isolation and published migration/provenance distinctions. No host-account shortcut.
- Swift: 1:23 setup/cache, 1:47 CLI build, 2:36 macOS installation process discovery, 1:23 Desktop build/tests, plus cleanup. Move independent macOS discovery proof off the serial Swift path; evaluate compilation reuse without erasing platform coverage. Current Xcode compile job is below five minutes in this sample.
- Linux lint, Python, website and smoke jobs are already below two minutes. Optimizing them first will not move the critical path.
- Main push already has exact merge-group proof reuse. Extending reuse from PR to queue needs proof of equivalent integrated content and workflow inputs; PR-head equality alone is insufficient. Retain queue integration coverage until that contract is demonstrated.
- Runner-platform comparison is pending clarification of “blackforge.” No paid platform selected or adopted. No recurring cost estimate is established. Sharding may increase billed minutes and artifact storage; measure those alongside wall time.

## Acceptance and review limits

No tests or jobs have been removed, demoted or changed. Release/nightly workflows are unchanged and have not been rerun by this investigation. No under-five-minute result is claimed. Required follow-up: inspect test history before removing coverage; benchmark each proposed change; capture ten consecutive real typical code PR runs with queue and push-to-merge timestamps; itemize remaining delay and cost if the target remains unmet.

Review finding: three independent jobs exceed five minutes, so a Rust-only change cannot satisfy acceptance. Separate Tasks should own Rust execution, disposable installation, and macOS critical-path work after the proposed direction is reviewed. Keep this Task responsible for the aggregate measurement.

## Work reduction evidence (follow-up)

The latest PR's 2,548 nextest cases sum to 4,148.43 test-seconds while execution wall time is 1,043.868 seconds: approximately four concurrent occupied test slots. The seven slowest total 685.29 test-seconds (16.5%); the top 100 account for 1,860.20 (44.8%). These sums include waiting and subprocess work, not CPU measurements. Removing every slow case would still leave substantial distributed fixture cost, and is not proposed.

GNU time reports 3,273.62 user CPU seconds plus 375.33 system seconds for the combined Rust build/test command, averaging 308% CPU. This is aggregate command accounting, not a per-test CPU profile. It establishes substantial computation alongside waiting; it does not establish an optimal core count.

`native_runner_is_quiet_until_its_turn_ends` explicitly waits 60 seconds for its Codex case (`rust/loopflow/src/ops/linear/task_planning_tests.rs:1610`). The behavior is important: planning should remain quiet until a turn boundary. Investigate whether a controlled clock or deterministic event boundary can preserve the assertion; faster hardware cannot remove that literal delay. Do not weaken the observation window without understanding the regression it guards.

Installation logs record a 2:42 initial build, a 53.18s target build, then additional 53.19s, 1:15 and 52.47s compilations interleaved with selected tests. Those three later compilations total about three minutes. They are an investigation target, not yet proven redundant: inspect Cargo fingerprint invalidation before changing the runner. The separate native-title proof changes build provenance intentionally and costs 49.53s compilation; retain its distinct authority.

## Runner economics (researched 2026-10-12)

Jack Heart selected doing less work first, then cost-efficient hardware. Repository visibility is PUBLIC. Standard public GitHub runners have free compute; larger runners are charged even for public repositories. Therefore Blacksmith's advertised savings relative to paid GitHub minutes are not a demonstrated saving here. [GitHub billing](https://docs.github.com/en/billing/concepts/product-billing/github-actions).

Blacksmith lists Ubuntu x64 from $0.004/minute, ARM from $0.0025, and macOS M4 from $0.08, with 3,000 free minutes/month advertised and an OSS application program. Its claimed 2x runtime improvement is not a Loopflow measurement. Exact size, free-tier eligibility, OSS terms and workload results are needed for a quote. [Blacksmith pricing](https://www.blacksmith.sh/pricing).

Cost comparison must use net charges, not gross metered usage before discounts. Keep subscription costs separate from Actions compute. No platform, paid trial or new infrastructure has been adopted. The first comparison should retain free standard runners while measuring fixture/build work eliminated. No release-signing migration is proposed; any future runner adoption must account for its GitHub App permissions and job-time secret execution, not assume that inability to query stored secrets means jobs cannot receive them. [Blacksmith security](https://www.blacksmith.sh/security).

## Confirmed build invalidation (2026-10-12)

`build.rs` reads `LOOPFLOW_BUILD_PROVENANCE` and `LOOPFLOW_MIGRATION_AUTHORITY`, then originally exported their defaults with `cargo:rustc-env` under those same names. Cargo supplies those values to running tests. Nested Cargo therefore observes different inputs from the parent that omitted them, and invalidates the build. A dependency-free reproduction and the real Loopflow fingerprint trace both identify `EnvVarChanged`.

On this macOS checkout, unchanged-source warm CLI builds measured 0.27s with the exported inputs and 42.57s when returning to the original omitted inputs. The repair gives compiled outputs distinct names while preserving external build inputs. After the initial source-change rebuild, equivalent nested/outer builds measured 0.16s/0.13s. This is a local mechanism demonstration, not a hosted CI speedup estimate. Distinct development/published builds still intentionally differ; this repair does not combine their authority.

## First measurement candidate

CI supplies its already-built candidate CLI through `LOOPFLOW_TEST_LF`, avoiding
per-process Cargo invocations in nextest fixtures. Disposable installation
preparation compiles selected targets together and uses the same candidate. A
library-only selection still explicitly builds the CLI; published title-hook
installation retains a separate published build. Ordinary focused Cargo tests
retain artifact discovery, including custom target directories. No test or job
is deleted or demoted, and no runner platform or recurring cost changes.

Local validation: default and explicit release/published build-info tests pass;
full disposable installation passes all nine cases in 84.37s, library-only
selection and its two migration cases pass in 104.75s, and published title-hook
proof passes in 41.83s. These include cached local container preparation, not
matched hosted measurements. Installation-runner Python tests, formatting and
Clippy pass. Hosted CI owns the materialized full matrix and performance result.
The prepared-CLI nextest path also passes all 29 Flow cases with four test
threads: 44.953s execution, 94.34s including test compilation on this macOS
checkout. That is a focused compatibility check, not a Linux CI comparison.

[Behavior-to-test audit](ci-test-coverage.md) and
[PR failure history](ci-failure-history.md) retain the independent research.
Jack Heart values removing unnecessary test code even when timing savings are
small, while preserving useful regression detection. Duplicate architecture
execution and brittle prose assertions are concrete separate cleanup candidates;
failure frequency alone does not justify deletion. The historical zero-test
installation selector also motivates rejecting empty proof selections.

The target remains under five minutes for required checks over ten consecutive
real typical-code PR runs, with merge queue and push-to-merged time reported
separately. One candidate run cannot satisfy it. Use its per-job/step result to
choose the next reduction; macOS compilation and remaining Rust fixture work
are still open. Independently deliverable further changes belong in separate
Tasks/PRs, retaining this Task's aggregate measurement obligation.

## Candidate observations and build-sharing experiment

PR #1543's first hosted candidate is
[run 38314949621](https://github.com/loopflowstudio/loopflow/actions/runs/38314949621),
head `3b124e6b943da363c34ff1b7991142f675d88678`. The full run passed. Swift took
4:24, installation 5:28, Xcode compilation 2:14, and Rust 17:49. Aggregate
non-skipped job occupancy was 34:29 versus 46:44 in the detailed baseline run;
these observations do not isolate runner/cache variation from the source change.
The candidate does not meet five minutes.

Rust compilation took 2:11 and nextest execution 886.890s (14:47), with all
2,548 cases passing and 17 skipped, matching the baseline counts. Test durations
sum to 3,516.38s versus 4,148.43s in the baseline, while execution wall time
falls from 1,043.868s. The combined build/test command used 2,855.76 user CPU
seconds and 349.40 system seconds. Four test slots remained well occupied;
this is still substantial test work after eliminating nested Cargo entry.

The longest remaining cases are watcher reconnect (156.008s), idle native
runner/no-poll (120.359s), and the resolution matrix (105.311s). Sharding alone
cannot erase the longest individual case, and a two-minute compilation plus
that case and artifact/setup overhead can still exceed five minutes. Investigate
fixture cost and scheduling order alongside any partition experiment.

Installation's managed step took 4:02: its consolidated cold compilation took
3:07, followed by nine executed one-test proofs. Subsequent Cargo calls each
took 0.15–0.17s instead of rebuilding. The separate published-title step took
1:19, including 1:09 compilation. Distinct provenance remains a real build
cost; reducing test assertions cannot remove that compile floor.

A local macOS nextest archive contains 60 binaries, including the fixture CLI,
and occupies 411,556,926 bytes (392 MiB). Packaging took 0.75s after compilation;
extraction took 3.24s. An isolated Flow proof passed using the extracted CLI
without recompilation (3.831s test execution; 27.19s total including archive
extraction and discovery across all binaries). The first extraction attempt
failed because its destination directory did not exist; creating that directory
resolved the setup error. This establishes local artifact usability, not Linux
size, hosted transfer time, or cross-machine path compatibility.

[Nextest build reuse](https://nexte.st/docs/ci-features/archiving/) can distribute
one compilation across ordinary test runners. The next comparison must include
build time, upload/download, extraction, discovery, partition balance and total
runner occupancy. The source revision and materialized migrations must match
the archive. No sharding or hardware change has yet been adopted.

Independent maintenance cleanup is captured in Infrastructure Task `lf-6d3ff35`,
“Remove duplicate and brittle test assertions,” in the loopflow repository. It
retains one architecture-check owner and meaningful behavior/safety assertions;
it does not own the under-five-minute acceptance or run workers automatically.

## Matrix fixture reuse experiment

The [historical timing audit](ci-timing-history.md) identified repeated fixture
seeding as a concrete regression. A matched local macOS experiment measured
`matrix_every_command_every_environment` at **53.62s before, 13.22s after**
restoring a prepared fixture for each case. All 70 command/environment cells
remain, with unchanged expected outcomes; no tests were removed or demoted.
Temporary instrumentation measured roughly 0.54s per original seed versus
0.02s per restore and was removed after measurement. This is local evidence,
not a measured hosted CI reduction.

The fixture seeds once, closes the SQLite store, and saves the complete Home
and repository. Every command receives independent restored bytes at the same
absolute paths recorded by the registry; its mutated files are removed before
the next case. No live database or mutable plan is shared. Review retained the
existing classifier limitation noted in the coverage audit: unknown downstream
errors can count as resolved. This optimization does not establish stronger
resolution coverage than the original test.

Raw measurements: `/tmp/ci-matrix-baseline.log` and
`/tmp/ci-matrix-snapshot.log`. The final three-test module passed in 12.64s;
formatting, Clippy with warnings denied, and diff checks passed.

## Remaining measurement priorities

Profiling the matrix's one fresh store measured 468ms initialization: 445ms
executing 72 canonical migrations, 460ms including their bookkeeping. Draft,
foreign-key and JSON validation added less than 1ms in this empty fixture.
Raw trace: `/tmp/ci-migration-profile.log`. Temporary instrumentation was
removed. This supports reuse of initialized fixtures; it does not justify
skipping real migration tests or changing immutable released SQL.

An installation-cache prototype bound Cargo target and registry directories
from the Mac host into Docker. Cold preparation took 166s, then a subsequent
command unexpectedly recompiled for 124s before one migration proof passed.
The next command rebuilt again; the experiment was stopped. An unchanged tiny
Cargo project reproduced an immediate rebuild on the bind mount, while native
Docker storage reused its output. The exact invalidation cause remains open.
The unused prototype was removed; Linux-native cold/warm measurement is still
needed before adopting caching. Raw logs are `/tmp/loopflow-installation-cache-cold.log`
and `/tmp/loopflow-{bind,native}-fingerprint*.log`.

Next candidates remain shared Rust compilation with archived test partitions,
moving both existing reconnect proofs to the existing smoke job, and Linux
installation dependency caching. At the first candidate's measured costs,
eight four-slot partitions still have a 304s optimistic floor including
compilation when the 156s watcher remains on that path. Separately owning
that target lowers the bound to 268s before artifact transfer and scheduling.
These are bounds, not predictions or a selected permanent runner count.
Measure total runner occupancy alongside elapsed time; no premium hardware
or reduced behavioral observation window is selected.
