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

## Shared-build experiment prepared

The next workflow candidate builds one archive, runs eight four-slot Rust hash
partitions, and uses partition zero for the existing Python checkout-refresh
proof against that CLI. Both Linux reconnect cases move intact to the existing
smoke job, whose integration build supplies the smoke CLI. The longest remaining
quietness proof starts first. Every partition and the shared build feed the
required aggregate; empty partitions fail and failed partitions do not cancel
their peers. Exact merge-group proof reuse remains unchanged.

The local archive contains 58 test binaries plus the CLI; the reconnect binary
is absent. Comparing complete nextest listings verified that all 2,546 locally
runnable cases occur exactly once across the eight partitions (306, 299, 351,
321, 322, 295, 320, 332). Local platform counts do not establish Linux coverage.
Matched historical cases sum to 391–451 seconds per partition before the newer
matrix fixture speedup; the 120s quietness case remains the longest single case.
These sums are scheduling inputs, not predicted elapsed or CPU time.

Validation: actionlint 1.7.12 passed; all 35 CI gate tests passed; a Flow proof
passed from the extracted archive/CLI in network isolation (17.925s). Archive
creation took 55.84s compilation plus 0.72s packaging on this Mac. Raw evidence:
`/tmp/ci-shard-*.{json,log}`. Hosted Linux transfer, setup, complete partition
execution, total runner occupancy and under-five-minute acceptance remain open.
Eight partitions are an experiment, not a cost-optimality claim.

## Second hosted candidate and macOS split

[Run 38324731529](https://github.com/loopflowstudio/loopflow/actions/runs/38324731529)
passed at PR head `2fe9c22467`, checking merged source `2168801dc9b4`.
The matrix fell from 105.311s to **10.085s**. Whole Rust occupancy was **18:26**:
129s compilation, 923.787s test execution, 2,551 passes and 17 skips. This is
not an unchanged-source comparison: the newer merged main replaced three old
test names with six new names, in addition to the fixture change. Runner
variation and other source changes prevent attributing the whole-job difference
to this optimization.

Installation passed in **5:17**, Swift in **8:02**, Xcode in **4:07**, and smoke
in **1:44**. Swift's macOS 15 ARM64 runner spent 147s building the CLI, 167s on
library-test compilation/execution for process discovery, and 70s on Desktop
build/tests; setup and cache restoration account for most of the remainder.
The next candidate moves the unchanged process-discovery command to its own
ordinary macOS job and requires it in the aggregate. This preserves both
required binaries and the deleted-executable regression while removing their
serial dependency. Extra job setup and cold-cache cost must be measured;
neither this split nor Rust partitions establish under-five-minute CI.

Validation of the added job: all 35 gate tests and actionlint passed. The first
lint retry met a transient Go proxy DNS failure; running the already-cached
actionlint module succeeded. Raw hosted log: `/tmp/loopflow-ci-baseline-lf12be237/38324731529.log`.

## First parallel run: transfer dominates

[Run 38331887738](https://github.com/loopflowstudio/loopflow/actions/runs/38331887738)
at head `3329a99020` failed only the Python migration-order assertion and the
aggregate. That assertion still looked for preparation in `rust-test`; it now
checks that materialization precedes compilation in `rust-build`. All Rust
partitions, checkout refresh, both reconnect proofs, and macOS jobs passed.
All 45 CI gate/release-automation tests pass after the correction.

Workflow elapsed was approximately 9:30. Build occupied 4:18; compilation took
206s, packaging 24.39s, and the uploaded archive was **2,124,316,446 bytes**.
Partition 1 spent 53.16s extracting before 95.101s executing tests; its total
job was 4:56. Installation took 4:05, smoke/reconnect 4:14, Swift 4:24,
macOS process discovery 4:31, and Xcode 4:09. Parallelization helped elapsed
time, but repeated transfer of a two-gigabyte archive is excessive work.

The next measurement sets the shared Rust build and partitions' dev/test
debug-info levels to zero. Optimization and debug assertions stay unchanged;
local profiles and release/nightly jobs are untouched. This trades source-line
detail in CI backtraces for smaller binaries. Its archive-size and timing gains
remain unmeasured, and the changed dependency profile can cause a cold build.
Raw log: `/tmp/loopflow-ci-baseline-lf12be237/38331887738.log`.

## Debug-symbol reduction: full green run in 5:24

[Run 38341497079](https://github.com/loopflowstudio/loopflow/actions/runs/38341497079)
passed at head `eb58dc3a59`. Run creation was 03:23:42 UTC on October 13;
the required aggregate finished at 03:29:06, **5:24** later. This is one
experimental PR run, not the ten consecutive typical-code runs required for
acceptance. Merge-queue and push-to-merge results remain unmeasured.

The archive shrank from 2,124,316,446 to **414,562,554 bytes**. Shared build
occupied **2:12**, including 108s compilation and 4.25s packaging. Partition
8 extracted it in 5.52s, versus 53.16s on the earlier measured partition.
Different source/cache/runner conditions prevent assigning the entire wall-time
improvement to debug symbols, but the artifact reduction is directly observed.

| Remaining path | Job occupancy | Finish after run creation | Measured work |
|---|---:|---:|---|
| Installation | 5:10 | **5:20** | Managed compilation 178s; published compilation 63s |
| Shared Rust build + partition 8 | 2:12 + 2:49 | **5:13** | Partition download 35s, extraction 5.52s, tests 114.918s |
| macOS process discovery | 4:16 | 4:31 | Check step 232s |
| Xcode | 4:04 | 4:20 | Below the target in this run |
| Swift | 3:41 | 3:58 | CLI build 94s, Desktop 61s |
| Smoke and reconnect | 3:46 | 3:56 | Both reconnect proofs retained |

Summed non-skipped job occupancy was **45:48**, compared with the original
**46:44** and first candidate's **34:29**. Parallel execution materially reduces
waiting, but has not demonstrated a comparable reduction in total runner use.
These are elapsed runner-seconds across Linux and macOS, not CPU-seconds or a
dollar estimate. Platform mix and changing source/cache state matter.

The next useful measurements concern installation dependency reuse on native
Linux storage and Rust artifact transfer. The earlier Mac bind-mount rebuild
does not establish Linux behavior. All eight Rust partitions currently receive
the full archive. Historical cases split into 1,950 library cases consuming
2,220 case-seconds and 596 other cases consuming 1,126 case-seconds after
excluding reconnect; this is insufficient evidence to choose separate archive
groups without measuring their payload sizes and balancing execution.

No coverage was removed. No broader local test rerun is needed for this evidence
update. Raw API records: `/tmp/ci-no-debug-{run,jobs}-rest.json`; artifact record:
`/tmp/ci-no-debug-artifacts.json`; build, installation and partition logs:
`/tmp/ci-no-debug-{build,installation,shard8}.log`.

### Installation cache experiment

A disposable native-Linux bind-mount probe compiled once in 0.51s and reported
`Fresh` in 0.00s on the next invocation. This separates native bind behavior
from the earlier Mac-host filesystem failure; it does not prove full-suite
reuse. The next hosted candidate exposes only Cargo target and registry through
`--cache-dir`, with an Actions cache keyed by the actual Docker image identity,
architecture, lockfile, workspace manifest and harness. The default local path
retains Docker volumes. Proof source and installation authority remain disposable.
Successful PR runs can populate their scoped cache, allowing cold/warm measurement
before landing. Cache hits never skip compilation or proofs. Hosted restoration,
save cost and net savings remain to be measured.

Validation: 51 installation-harness, CI-proof and release-automation tests passed;
actionlint passed. Full native-Linux cache execution is assigned to hosted CI.

[Cold run 38350155158](https://github.com/loopflowstudio/loopflow/actions/runs/38350155158)
passed in **5:44**, at head `58a2ce5a02`. Installation occupied 5:27:
managed compilation 183s, subsequent Cargo commands 0.15–0.16s, and published
compilation 64s. Native Linux bind storage therefore reused the build within
the job. Cache save nevertheless failed: registry source files owned by the
container user denied the runner's tar read. The next candidate returns ownership
of this disposable cache to the runner before upload. No warm-cache saving has
been demonstrated. It also lets main populate a missing cache despite exact
merge-proof reuse, matching the existing Rust cache-warming behavior.

### Smaller archives with the same nine execution jobs

Local recompression at Zstandard level 6 saved 7% (411MB to 383MB) but added
about five seconds over level 3; it was not selected. Instead, library and
integration binaries can travel separately. Actual nextest archives contain
**87,365,298 bytes** for libraries and **374,203,128 bytes** for integration
targets, each with its CLI and supporting files. Compilation ran once; the
second archive command reused it in 0.68s. Packaging took 1.29s and 3.12s.
These are local Mac payloads, not hosted Linux timing predictions.

The next matrix uses five library and three integration partitions. Partition
zero runs the unchanged long quietness proof and checkout refresh against the
library archive. This avoids a heavy fifth library partition while retaining
the same nine execution jobs. Local enumeration assigns all **2,546 runnable
cases exactly once**, with no missing or extra cases and exactly one quietness
proof. Archive metadata contains only its selected binary kinds and both
archives include the CLI. The superseded quietness scheduling priority is removed.
Expected downloaded bytes fall from nine full archives to six library and three
integration archives; actual hosted balance and transfer remain to be measured.

Validation: all 45 CI-proof/release-automation tests and actionlint pass after
regrouping and cache-warming changes. One library metadata proof and one Flow
wire-contract proof passed from the separate extracted archives with networking
blocked. Raw grouped listings: `/tmp/ci-group-*.json`; actual
archives: `/tmp/ci-actual-{library,integration}.tar.zst`; cold hosted installation
log: `/tmp/ci-install-cache-cold.log`.

## Grouped hosted run: Linux paths below five, Swift remains

[Run 38351066881](https://github.com/loopflowstudio/loopflow/actions/runs/38351066881)
passed at head `02450d1fdd`, in **6:32** from creation to required aggregate.
The library archive is **75,729,360 bytes** and integration archive
**379,860,559 bytes**. Six library downloads plus three integration downloads
total about 1.59GB, versus 3.73GB for nine prior full archives: **57% less**.
Shared compilation took 117s; the second archive reused it in 0.10s. Packaging
took 1.44s plus 3.41s. All Rust execution jobs finished within **4:55** of run
creation, including quietness and checkout refresh. Individual partition finish
times were 4:15–4:55.

Installation finished within **4:28**, on a cache miss. Its managed build took
134s, subsequent Cargo calls 0.10–0.11s, and published build 46.28s. Cache save
now succeeded, after returning ownership, in the six-second post-action step.
This proves cache creation, not warm restoration. The difference from the prior
183s/64s compilations is runner variation, not a cache benefit.

Swift finished at **6:28** and set the critical path: Rust toolchain installation
81s, Rust cache restoration 23s, Swift cache restoration 25s, CLI build 133.10s
(148.02 user, 13.55 system), and Desktop build/tests 74.99s (22.85 user,
9.21 system). Other macOS jobs finished by 4:23. The next investigation is
macOS toolchain/setup variance and repeated CLI compilation; merely improving
Linux further will not resolve this observed run. Summed job occupancy was
**46:38**, still near the original 46:44. Neither five-minute acceptance nor a
large total-work reduction is demonstrated.

Raw evidence: `/tmp/ci-grouped-{jobs,artifacts}.json`,
`/tmp/ci-grouped-{build,task-installation,swift-test}.log`.

### Next macOS measurement

The slow Swift runner upgraded Rust 1.98.1 to 1.99.0, downloading five components
because its preinstalled stable toolchain included Clippy and rustfmt. No
toolchain-selection change is selected from one slow download. The next candidate
sets dev/test debug-info levels to zero in Swift's CLI build and the independent
macOS process-test job. Debug assertions, optimization and coverage are unchanged;
CI backtraces lose source-line detail as on Linux. This may start with cold
dependency caches for the changed profile. Local and release profiles are untouched.
The same run can measure restoration of the now-populated installation cache.

Validation: 45 CI-proof/release-automation tests and actionlint pass; hosted macOS
compilation and timing are the remaining verification.

The following run, `38352209666`, verified installation-cache restoration:
539,558,615 bytes restored, three crates compiled across both candidates,
managed compilation 65s and published compilation 42.80s. Installation finished
3:21 after creation. Its ordinary proof invocations reused builds in 0.10–0.11s.
In contrast, the shared archive job again compiled 256 crates, taking 159s
before packaging. Its 3:21 finish plus the unchanged 120s observation floor
already excludes five-minute completion on that runner.

The next candidate lets the shared Rust build and both macOS Rust jobs save
missing dependency caches during PR review, rather than requiring a main run
first. Main remains the shared source for other branches. Existing up-to-date
caches are not saved again; the action retains its normal workspace-output
cleanup. No execution is skipped on a PR cache hit. This targets repeated
dependency compilation and allows a controlled cold/warm comparison before
landing. The first seeding run can still be cold and pay upload overhead.

Validation: 45 CI-proof/release-automation tests and actionlint pass for the
cache policy change. Raw build/installation logs:
`/tmp/ci-macos-{rust-build,task-installation}.log`.

Run `38352209666` ultimately passed in **5:59**; Swift finished at 5:20 and
the final Rust partition at 5:54. Swift's changed profile compiled 258 crates
in 163.46s, while its actual SwiftPM build reused outputs in 1.13s and 503 tests
ran in 63.625s. This isolates the remaining repeated Rust dependency work from
Swift source compilation.

The cache-seeding run `38352858853` passed in **6:57**. The shared Rust job
uploaded 310,652,887 cache bytes and macOS process tests uploaded 299,789,746.
The Xcode job exposed a third macOS Rust build: its app script invokes Cargo
for the bundled CLI in `target/dev-app-control`. That job spent 71s updating
Rust and 258s in build-for-testing. The next candidate applies the same no-debug
profile and PR cache-seeding policy there. The other three changed build jobs
can now consume their seeded caches. This keeps the app's bundled-CLI compile
proof, signing settings and all tests unchanged.

Validation: 45 CI-proof/release-automation tests and actionlint pass. Hosted
Xcode cache creation and subsequent warm full-workflow timing remain unproved.

Run `38353681649` passed in **6:09**. The warmed shared Rust build finished
within 2:42, Swift within 3:42, and macOS process tests within 3:19. The Xcode
job's newly cold profile finished at 6:03. Its cache is now available for the
next comparison. Installation finished at 4:07. The last integration partition
finished at 5:11, with 117.363s test execution averaging 305% CPU; the quietness
job finished at 5:09. Hardware and workload are unchanged by the following
scheduling experiment.

### Overlap existing quietness cases and use available execution slots

The native quietness proof contained four independent fixture invocations in
one sequential loop. It now exposes each existing case as a named test with
one shared assertion body: foreground Codex, foreground Claude, task-bound
headless Claude and taskless headless Claude. The 60-second Codex observation,
the three one-second Claude observations, post-turn acquisition and every
assertion are unchanged. Partition zero runs the four tests in separate nextest
processes, retaining the fixture's environment isolation. No production code
changes for this split.

Local execution passed all four cases in 92.724s, including a 92.719s Codex
case. Nextest reported one leaked output handle for foreground Claude; its
focused standalone diagnostic passed in 18.688s without a leak. The intermittent
local warning's cause is unconfirmed, not declared fixed. Hosted Linux execution
is the next check. All 2,549 local runnable cases are assigned exactly once;
the three additional names expose existing cases rather than adding behavior.

The eight ordinary partitions move from four to six test slots on the same
runners, addressing measured waiting without adding jobs. A six-library-shard
alternative was inspected but had worse estimated balance; it was not selected.
The fleet-receipt exclusivity override continues to reserve all configured slots.

Validation: `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, the four
focused proofs, 45 CI-proof/release-automation tests and actionlint pass, with
the local nextest warning above retained. Raw evidence:
`/tmp/ci-quietness-{split,foreground,clippy}.log`, `/tmp/ci-split-library-*.json`,
`/tmp/ci-xcode-integration1.log`.

## First complete run below five minutes

[Run 38362344076](https://github.com/loopflowstudio/loopflow/actions/runs/38362344076)
passed at head `02eeedce51` in **4:56**, from creation at 07:23:27 UTC on
October 13 to the required aggregate completing at 07:28:23. This was the full
PR workload, not a checkpoint deferral or selective rerun. It proves one
sub-five-minute run with four seconds of margin; ten consecutive real typical-code
runs, merge-queue time and push-to-merged remain unproved.

| Required path | Finish after creation |
|---|---:|
| Shared Rust build | 2:26 |
| Last Rust partition | 4:50 |
| Quietness and checkout refresh | 4:17 |
| Installation | 4:02 |
| Swift | 4:25 |
| Xcode app and test-runner build | 4:26 |
| macOS process discovery | 2:42 |
| Smoke and reconnect | 4:34 |
| Required aggregate | **4:56** |

The shared build compiled two crates, not 256, and retained its up-to-date cache.
All four hosted quietness cases passed without a leak warning in **71.577s**:
Codex 71.575s and Claude 12.616–12.802s. Their original observation windows and
assertions remain intact. The local Mac leaked-handle observation is not
declared diagnosed merely because Linux passed cleanly.

Summed runner occupancy was **42:08**, versus the original **46:44**: about 10%
less. It exceeds the first serial reuse candidate's **34:29**, so parallelism
buys much of the elapsed-time reduction with additional setup/transfer work.
These are mixed-platform runner-seconds, not CPU time or dollar charges. No
premium hardware, paid runner platform or coverage removal was introduced.

Raw job records: `/tmp/ci-quietness-jobs.json`; logs:
`/tmp/ci-under-five-{build,quietness,library1}.log`. The implementation remains
published for review; merge is not armed and Task completion is not requested.

### Confirmation caught an incidental ordering assertion

[Run 38369321555](https://github.com/loopflowstudio/loopflow/actions/runs/38369321555)
finished in 4:52 but failed
`local_planning::rotation_saves_membership_and_pending_effects_offline`.
The test assumed the started Task occupied the first preview row. Task listing
orders by descending update time; when backlog is newer, its `historical`
disposition appears first instead of the started Task's `move` disposition.
This failed run does not count toward consecutive successful acceptance.

Both preview and applied-result assertions now select the started Task by ID.
The fixture deliberately makes backlog newer, so incidental creation timing
cannot conceal the assumption again. Existing checks for moved work, retained
backlog, rollback, pending delivery and execution preservation remain. No test,
assertion or production behavior is removed, and no retry is introduced.

The focused proof passed all four connected/mapped combinations in 58.352s
locally with external networking blocked; formatting and Clippy with warnings
denied pass. Raw evidence:
`/tmp/ci-confirmation-library1.log`, `/tmp/ci-rotation-order.log`.

### Confirmation caught a same-second timestamp assumption

[Run 38384646308](https://github.com/loopflowstudio/loopflow/actions/runs/38384646308)
failed `incoming_removal_retains_task_and_prevents_first_placement` at head
`c2ac0d29e6`. Observing deletion correctly increments the planning revision and
updates `updated_at`; the test adjusted only the expected revision. It passed
when creation and observation shared a second, then failed across a second
boundary. Every other Task field matched.

The fixture now starts with an old timestamp and explicitly proves advancement
on deletion observation. It compares every other Task field at that boundary,
then compares the entire post-observation Task before and after rejected
placement. Empty PR, event and Workflow checks remain. No production change,
retry, sleep or test removal is involved.

Validation: the focused proof with external networking blocked, `cargo fmt` and
`cargo clippy --all-targets -- -D warnings` pass. Evidence:
`/tmp/ci-removal-time.log`, `/tmp/ci-removal-clippy.log`,
`/tmp/ci-rotation-failure-115254323494.log`.

## Margin after the timestamp repair

[Run 38402785080](https://github.com/loopflowstudio/loopflow/actions/runs/38402785080)
passed at `1e9eb57c02` in **5:31**. Shared build finished at 2:43, the final
integration partition at 5:26 and the aggregate at 5:31. Compared with the 4:56
pass, build/archive rose from 109 to 124 seconds and that integration partition's
artifact download rose from 4 to 31 seconds. Its test step fell from 114 to 112
seconds. Swift finished at 4:54, installation at 4:18 and the last library
partitions at 5:07. Summed runner occupancy was 43:23. One fast pass has not
established enough margin or consecutive-run acceptance.

### Link ordinary integration tests once

The remaining integration archive contains dozens of executables that repeat
linked application/dependency code. A local prototype combined 48 targets, with
exact equality of all 481 collected case names after adding their former binary
name as a module prefix. Its archive was 77,573,429 bytes with local debug info;
the prior integration archive was 374,203,128 bytes without it. This directional
comparison motivated a hosted experiment; it is not a matched Linux saving.

The final layout combines **41 targets and 369 local cases** in `integration.rs`.
Fourteen dedicated targets remain: seven installation/release/reconnect/live
entrypoints and seven suites that directly mutate process environment. Keeping
the latter separate preserves their ordinary libtest isolation. Existing test
bodies, support modules, platform gates and ignored cases are unchanged.
Nextest still starts a separate process for every case. Cargo's explicit target
list and the integration module list register all 55 original integration source
files exactly once. Repeated `mod support` declarations become imports from one
shared fixture module; fixture implementations and assertions are unchanged.
Focused command examples now select the corresponding integration module.

A larger Zstandard window was also measured: recompression reduced the old
374 MB integration archive to 252 MB in 3.5 seconds locally, but still packages
and transfers repeated linked code. It was not added to the workflow; the
consolidated executable addresses that repetition at its source.

Validation: all 369 consolidated cases passed in network-isolated nextest
(447.574s locally). After replacing duplicate fixture declarations with one
shared module, four representative context/Session/planning/publication proofs
passed (14.350s); formatting and all-target Clippy with warnings denied pass.
Collected names and ignored flags match the original inventory exactly. Hosted
Linux collection, archive size and full timing remain the next evidence.

### Hosted consolidation result and remaining waits

[Run 38422834854](https://github.com/loopflowstudio/loopflow/actions/runs/38422834854)
passed at `19544d4047` in **5:21**. Integration archive bytes fell from
379,860,559 to **196,456,520**, about 48%; the library archive is 75,676,088
bytes. Total downloads across the unchanged nine jobs fall from approximately
1.59 GB to **1.04 GB**. Build/archive steps fell from 124 to 110 seconds,
but job setup, post steps and completion overhead left build completion at 2:46.
Every integration partition finished by 4:57; the last library partition
finished at 5:15 and the aggregate at 5:21. Runner occupancy was
43:36; no coverage was removed.

The last library partition ran 390 cases in 119.174s at 343% CPU utilization
on the four-CPU runner. The next candidate uses ten rather than six test slots
on the same runners to fill waiting time; this is a measured scheduling
experiment, not a claimed gain before CI. The exclusive fleet-receipt override
continues to reserve all configured slots.

Partition zero spent 74 seconds on quietness, then 31 seconds on checkout
refresh. The next candidate overlaps these independent, network-isolated
commands on the same runner. Each retains its command and assertions; the
shell waits for both, prints checkout output and returns failure if either
command fails. Four quietness workers and observation windows remain unchanged.

Validation: 45 CI-proof/release-automation tests and actionlint pass. A shell
check exercised both-success and each/both-failure combinations under errexit;
only both-success returned zero. Rust sources are unchanged from the full hosted
pass. The next full run owns concurrency and elapsed-time acceptance.

### Reject ten slots; remove machine-speed assumptions from two proofs

[Run 38424417775](https://github.com/loopflowstudio/loopflow/actions/runs/38424417775)
finished in **4:39 but failed**, so it provides no successful acceptance run.
The limiting library partition changed only from 119.174s at six workers to
117.204s at ten (343% to 360% CPU). Ten slots did not establish a worthwhile
benefit and are reverted to six. Quietness plus checkout refresh passed while
overlapping; that scheduling change is retained. Shared-build completion was
2:13 rather than 2:46, so the entire elapsed improvement cannot be attributed
to worker count or overlap.

Two timing assumptions failed:

- `rate_limit_pauses_independent_clients_and_resumes_after_reset` gave server
  startup and two HTTP calls a 250 ms deadline. Correct expiration could let
  the second call succeed. The fixture now installs a future reset, verifies
  shared pause and the stored deadline, then writes an expired nonzero deadline
  and verifies resumption. It moves beside the cooldown owner to access its
  private fixture path; no production API, retry or longer sleep is added.
- `a_task_committed_elsewhere_appears_once_and_bursts_converge` received six
  planning frames instead of at most five. Twenty transactions can legitimately
  span more debounce windows on a busy runner. **The incidental five-frame cap
  is removed**, replaced with strictly advancing Task counts (no repeated or
  stale snapshots), final count 21, and the retained full comparison to a fresh
  public read. This no longer claims a fixed frame count for an unbounded write
  duration; it preserves convergence and strengthens duplicate-state detection.

GitHub public PushEvents also supplied dispatch evidence: `02eeedce51` was
pushed at 07:23:23 UTC and its successful required checks finished at 07:28:23,
so the earlier **4:56 Actions run was 5:00 from push**. `19544d4047` was pushed
at 15:31:36; its run began at 15:31:42, six seconds later. Future acceptance
reports must retain both clocks rather than spending that dispatch margin.

Validation: both network-isolated focused proofs pass; the final expired-nonzero
deadline case was rechecked after refinement and passed in 0.165s. Formatting
and all-target Clippy with warnings denied pass. The watcher check also passed
its full fresh-read comparison; its local 156.683s included rebuilding the CLI
fixture, not a longer observation window. No retry was introduced.

### Full 4:55 pass; overlap the bundled CLI build with Swift

[Run 38427516700](https://github.com/loopflowstudio/loopflow/actions/runs/38427516700)
at `22c2afe45b` passed the complete required workload in **4:55** (16:04:45–
16:09:40 UTC, October 13). All Rust partitions finished by 4:32; Xcode finished
at 4:47 and the aggregate at 4:55. Its exact PushEvent timestamp was not found;
publication began at 16:04:25, an upper bound of 5:15 through completion rather
than a measured push-to-checks interval. Ten consecutive real runs, merge queue
and push-to-merged remain unproved.

Xcode's build step took 190 seconds. The app's post-build script began about
33 seconds into it and spent 150 seconds compiling the bundled Rust CLI.
The next candidate makes that build a macOS aggregate target independent of the
Swift framework, with the app depending on both. CI explicitly enables parallel
targets. The app still copies and signs the CLI before its own signing; the
same Python command supplies development provenance and validation-only migration
authority. No test, build output or signing requirement is removed.

This changes the app's Xcode build graph, including callers outside CI; release
and nightly workflow files remain unchanged. XcodeGen accepts the graph, and
the aggregate and app both resolve macOS deployment target 15.0 and the same SDK.
The headless app/unit/UI-runner build passes; strict helper and deep app signature
verification pass. Forty-five CI-proof/release-automation tests and actionlint
pass. Hosted timing, including the changed Xcode cache fingerprint, owns the
performance result; the local build is correctness evidence only.


The experiment is **rejected**. [Run 38430532823](https://github.com/loopflowstudio/loopflow/actions/runs/38430532823)
at `85c84ec51e` passed but took **5:44 Actions / 5:49 push-to-checks**.
GitHub PushEvent `23890631726` records 16:26:31 UTC, workflow creation was
16:26:36 and the aggregate completed at 16:32:20. All Rust partitions finished
by 4:35; Xcode finished at 5:39. Its changed source key missed the DerivedData
cache, and the build took 276 seconds. The CLI compile took 152 seconds;
Swift compiler output followed its completion. Buffered logs limit exact overlap
attribution, but neither elapsed time nor the log establishes the intended saving.
The aggregate target and parallel-target flag are removed, restoring the previous
app build graph. The extra configuration has not earned its place.

Run 38430369254 was a scratch-bearing checkpoint: all product jobs skipped.
It is excluded from timing acceptance. The retained status note was cleared after
its findings were preserved here. The prior 4:55 full pass consumed **40:19**
of mixed-platform runner occupancy versus baseline 46:44; this is not a dollar
cost estimate. The remaining proof is repeated real-run reliability and delivery,
not ten reruns of a warmed candidate. Merge remains Jack Heart's pending choice.

### Main synchronization preserves new integration coverage

Jack Heart's October 13 “keep going” and “go” authorize delivery of #1543.
Arming synchronized newer main at `35f3b184df`. Its new `pr_review_tests.rs`
and `scratch_tests.rs` were outside the consolidated target's module registry.
The PR was returned to reviewer-owned merge while repairing that gap. Both
modules now use the shared support module and all five new cases pass under the
network-isolated runner. Formatting and all-target Clippy pass.

CI archive preparation and the local Rust plan now check the source inventory
and reject unregistered top-level test files. A temporary omitted test source
confirmed rejection; the complete current inventory passes. Forty-five
CI-proof/release-automation checks, actionlint and Python lint pass. This adds
coverage from main rather than treating a silently omitted suite as a speed win.

### Merge queue exposes a disposable-remote maintenance race

The repaired PR head passed [run 38444759381](https://github.com/loopflowstudio/loopflow/actions/runs/38444759381)
in **4:34**, including the newly registered cases. Its first merge-queue run,
[38445438182](https://github.com/loopflowstudio/loopflow/actions/runs/38445438182),
failed `release_tag_is_idempotent_when_remote_tag_matches_head`: the immediate
second tag observed the first tag's release lock still held.

A disposable Git 2.55 experiment reproduced a push returning while a child
retained its inherited flock: **3 of 40** samples with receiver maintenance,
**0 of 40** with `receive.autogc=false`. Git's
[configuration reference](https://git-scm.com/docs/git-config#Documentation/git-config.txt-receiveautoGc)
documents automatic receiver maintenance. `TestRepo` now disables it on its
disposable bare remote. The test still performs real tag/push operations and
checks the remote commit and immediate idempotency. Production release locking,
published remotes, observation windows and retries are unchanged.

Validation with Git 2.55: tag idempotency, conflicting-tag refusal and surviving
publisher-child exclusion all pass (3 cases, 0.347s). Formatting and all-target
Clippy pass. The full hosted matrix owns remaining fixture effects.

The queue also missed the new Rust dependency caches for both the shared build
and bundled app CLI; PR-scoped caches were unavailable there. The shared build
compiled for 174 seconds and finished at 4:02 from run creation. Xcode restored
its signed DerivedData but spent 256 seconds compiling the CLI, finishing at
6:25. These are measured cold-cache costs, not a successful sub-five queue proof.
The first main run must populate the new cache namespaces; subsequent real PR
and queue observations are still needed. No warmed rerun is substituted for
that acceptance.

## October 13 follow-up: shell overlap rejected before publication

Jack Heart authorized lf-ddf2d53 to continue measured macOS critical-path work.
The supplied latest real queue, [38452715134](https://github.com/loopflowstudio/loopflow/actions/runs/38452715134),
remains **6:24 creation-to-aggregate completion**; #1543 is merged. No new
hosted acceptance run, PR or merge is claimed by this experiment.

The prototype started the exact development/validation-only app CLI build next
to ordinary signed Xcode `build-for-testing`. One private function built the CLI
for preparation and the existing copy/sign phase. Cargo compiled `loopflow`
once per sample; the post-build invocation waited on Cargo's lock and reported
`Fresh loopflow`. Process snapshots observed simultaneous rustc and
swift-frontend work, rather than inferring overlap from buffered Xcode logs.

Matched snapshots were restored at the same checkout, Cargo target and
DerivedData paths before every sample. Source-cold retains compiled Cargo
dependencies and downloaded Swift package sources, but removes workspace and
Xcode compilation outputs. Warm restores identical complete outputs, then
changes AppearanceMode's Light label and a Rust error message, with identical
contents/timestamps in both variants. These temporary source edits were restored;
none is a typical-code PR or ten-run acceptance sample.

| Inputs and execution order | Ordinary Xcode, seconds | Overlap including settings/preparation/cleanup, seconds | Saving, seconds |
|---|---:|---:|---:|
| Source-cold: baseline then overlap | 85.311 | 45.643 | 39.668 |
| Source-cold: overlap then baseline | 60.298 | 46.845 | 13.453 |
| Changed-source warm: overlap then baseline | 33.841 | 30.868 | 2.973 |
| Changed-source warm: baseline then overlap | 36.835 | 40.225 | -3.390 |

Both cold pairs improve, but the warm advantage does not survive reversed
order: combined warm overlap takes **0.417 seconds longer**. The required useful
cold/warm gain is not established. The first cold saving also shrinks sharply
with reversed order; do not attribute all 39.668 seconds to the mechanism.
Local machine contention and shared Xcode service caches remain confounders.
The prototype and workflow wiring are removed; ordinary CI, tests, signatures,
compiled authority, caches and job conditions retain their previous behavior.

Inputs: source baseline `160adc7e6fe7f04d8a707003e8af04d3971e9b94`, rustup
rustc 1.99.0 (`b940084d7`, aarch64-apple-darwin), Xcode 26.2 (17C52), SDK 26.2,
deployment 15.0, dev/test debug=0 and unchanged Cargo.lock. Initial empty compiled
outputs took 130.787s; package sources were already available, so this is not a
fresh network-cache timing. An early mismatched Homebrew 1.98.1 preparation
sample rebuilt dependencies and was excluded. Matching the app shell's existing
Cargo search path eliminated that mismatch.

Native preparation `(real/user/sys seconds; peak RSS bytes)` was
`32.70/37.34/1.40;3446652928`, `33.20/39.81/1.35;3440082944` for cold,
and `29.18/37.27/1.08;3443621888`, `38.46/36.77/1.46;3439296512` for warm.
Xcode's native user/sys ranges were 1.11–1.58/0.38–0.60s cold and
0.77–0.81/0.23–0.26s warm; peak RSS was 198131712–200081408 bytes.
Those Xcode figures exclude the shared build service and its compilers; they
are not total compilation CPU/RSS or evidence of lower runner cost.

Real cancellation exposed a compiler outside the owned process groups:
Xcode returned BUILD INTERRUPTED while a running swift-frontend still emitted
Darwin.pcm into this build's DerivedData. Scoped cleanup subsequently passed a
real simultaneous Rust/Swift cancellation check with status 143 and no remaining
checkout compiler. Eight focused process tests passed during the prototype,
including preservation of pre-existing compiler work and other output paths.
A candidate interrupted by concurrently running process fixtures was excluded;
fixtures were moved to disposable source/output roots. Full gate, signature and
runtime acceptance and hosted/queue verification were not performed after the
performance rejection. No installed Home or Applications contents were modified.

The locally checkpointed prototype is
`34c40623b19d7176a3f8e452aa2bac9c8e753013`; raw timing/process logs and snapshot
scripts use `/tmp/macos-ci-*`. The remaining blocker is a reliable macOS margin
on changed-source builds, without adding unjustified scheduling/cleanup
complexity. lf-ddf2d53 remains unfinished; lf-12be237 still owns the independent
ten-real-run, queue and push-to-merged demonstration. Five minutes is not waived.


## October 13 follow-up: control CLI code-generation trial rejected

Jack Heart authorized the targeted follow-up after shell overlap failed. This
iteration measured the existing control CLI rather than repeating overlap. Source
`38e0b19b6f160de356d2270f0c5c000bff6422a4`, unchanged Cargo.lock, rustup
rustc 1.99.0 (`b940084d7`, aarch64-apple-darwin), dev/test debug=0,
nonincremental ordinary dev optimization, development provenance and
validation-only migration authority. Xcode remains 26.2 (17C52), SDK 26.2;
this trial measures its serial Rust helper segment, not an Xcode build.

The initial empty compiled target built in **55.66s** with downloaded registry
sources available. Cargo's stable `--timings` report assigns the library 26.47s:
22.29s frontend, 4.18s code generation. The build-script run initially took
8.57s, then 0.62–0.70s in matched trials; its initial elapsed cost is not assumed
to recur. More backend parallelism therefore has limited headroom.

Trial: package-only `profile.dev.package.loopflow.codegen-units=256`, passed
through Cargo's `--config`; no tracked profile or production script was changed.
It retains optimization, assertions, source, target, build-script authority and
all dependency profiles. The first unmatched candidate library took 24.98s
(21.22s frontend, 3.76s code generation), which was insufficient for adoption.

| Matched inputs and order | Ordinary CLI + copy/sign, seconds | 256 units + copy/sign, seconds | Saving, seconds |
|---|---:|---:|---:|
| Source-cold: baseline then candidate | 30.644 | 32.945 | -2.301 |
| Source-cold: candidate then baseline | 28.695 | 28.230 | 0.465 |
| Changed-source warm: candidate then baseline | 31.333 | 29.853 | 1.480 |
| Changed-source warm: baseline then candidate | 34.438 | 32.258 | 2.180 |

Cold pairs total **1.836s slower**; warm pairs save **3.660s**. Cold improvement
is neither consistent nor positive overall, so the required cold/warm outcome
fails. No numeric criterion is waived. The first cold candidate also spent 2.98s
compiling the build script versus 0.50–0.52s in the other cold samples; the whole
segment counts, without excluding this contrary sample. Concurrent unrelated
Rust work and machine load limit attribution. Frontend elapsed varies
21.49–26.56s and code generation 3.60–4.77s; these are Cargo phase boundaries,
not an independent compiler CPU profile.

Reproduction: use the same target path and toolchain as the Xcode post-build
script, `target/dev-app-control`; set `CARGO_PROFILE_DEV_DEBUG=0`,
`CARGO_PROFILE_TEST_DEBUG=0`, `LOOPFLOW_BUILD_PROVENANCE=development`, and
`LOOPFLOW_MIGRATION_AUTHORITY=validation_only`. Build with:

```sh
cargo build --locked --bin lf --target-dir target/dev-app-control --timings
cargo --config profile.dev.package.loopflow.codegen-units=256 build \
  --locked --bin lf --target-dir target/dev-app-control --timings
```

After dependency preparation,
`cargo clean -p loopflow --target-dir target/dev-app-control` produces a
source-cold snapshot. Restore that identical
snapshot with APFS `cp -cRp` at the same path before each cold sample. Capture
separate complete ordinary/candidate output snapshots for warm samples, then
restore the corresponding original snapshot before each warm sample. Replace
`git could not resolve {upstream}` with `CI benchmark: git could not resolve
{upstream}` in `rust/loopflow/src/build_info.rs`, with identical contents and
nanosecond timestamps for both variants. This forces real current-source
library/CLI compilation. Time Cargo, copy the resulting `debug/lf` into a
disposable bundle, and ad-hoc sign it with `codesign --force --sign -`. Input
snapshot restoration is experimental setup outside the timer; the candidate
adds no production preparation or background cleanup. All eight samples built
and signed successfully; final strict helper signature verification passed.
Native Cargo `(user+sys)` totals: cold ordinary 78.31s / candidate 77.92s;
warm ordinary 80.23s / candidate 80.15s. Peak RSS was 3.44–3.45 GB ordinary,
3.23–3.25 GB candidate. These are local process-tree figures, not hosted cost.

Raw records and the replay script remain at
`/tmp/macos-ci-codegen-snapshots/results.jsonl`,
`/tmp/macos-ci-codegen-bench.py`, and `/tmp/macos-ci-codegen-*.log`.
Temporary Rust source edits were restored; the ordinary output snapshot was
restored; no compiler from this checkout remained. No installed Home or app was
modified. No trial profile, wrapper or duplicated production implementation
remains, and no test was removed.

Review finding: backend parallelism does not establish useful source-cold
margin, while frontend compilation dominates this segment. Reject before
changing the Xcode cache namespace or adding configuration. Full Xcode timing,
app signatures, runtime acceptance, gate, hosted CI and verified delivery were
not attempted for this rejected candidate. The next mechanism needs a small
frontend work reduction with reliable matched cold/warm proof. lf-ddf2d53 remains
unfinished; lf-12be237 retains ten-real-run, queue and push-to-merged acceptance.


## October 13 follow-up: nested planning future attribution

The next source-level trial follows Jack Heart's authorization for measured
macOS CI margin. Baseline `d27b81c14dee724171a209f5fc8216d06b0d8fd5`, unchanged
Cargo.lock, rustup rustc 1.99.0 (`b940084d7`, aarch64-apple-darwin), dev/test
debug=0, nonincremental dev optimization. No overlap, backend override, test
deletion or cache identity change participates.

The library was profiled with rustc's [self profiler](https://doc.rust-lang.org/beta/unstable-book/compiler-flags/self-profile.html),
using `cargo rustc -p loopflow --locked --lib --target-dir target/dev-app-control
-- -Zself-profile=<output> -Zself-profile-events=default,args -Ztime-passes`
and diagnostic-only `RUSTC_BOOTSTRAP=1`. Official `rust-lang/measureme` tools
decoded the trace. Those flags are absent from production and measured builds.
Query-string recording alone cost 23.07s baseline / 23.23s candidate: profiled
elapsed time is explicitly excluded from performance acceptance.

Attribution found dispersed work: 17,898 type-check roots and 16,159 borrow-check
queries, with no single multi-second source function. The largest borrow-check
query was Flow `run_skill` (208.114ms); the largest instance-analysis query was
Flow `run_command` (223.758ms). Both carry the nested planning future returned by
`ops::linear_observe::sync_repository`.

Trial: replace that function's concrete async return with a `BoxFuture`, keeping
its body in one `Box::pin(async move { ... })`. No wrapper or second implementation.
Callers still await the same work with the same deadlines and drop cancellation;
the cost is one allocation and dynamic polling per sync. This moved the caller
queries to 4.540ms and 1.398ms respectively. The sync function's own borrow check
rose from 5.913ms to 15.113ms. Total self-time for obligation evaluation fell
1.60s → 1.22s and instance analysis 1.14s → 0.979s, but total type checking and
borrow checking increased. These diagnostic observations establish attribution,
not a reliable end-to-end gain.

Matched builds used SDK 26.2 (Xcode 26.2/17C52), deployment 15.0, and explicit
development provenance / validation-only migrations. Preparation compiled the
ordinary dependency set once (39.75s, excluded experimental setup); both variants
then restored one identical source-cold snapshot at `target/dev-app-control`.
Warm samples restored each variant's full output snapshot before the same
`build_info.rs` error-message edit, with matching contents/timestamps. The only
variant source difference was the boxed future. Ordinary Cargo flags, copy and
ad-hoc signing were timed together. No candidate preparation or background cleanup
exists; snapshot resets are experimental setup, outside both timers.

| Inputs and execution order | Ordinary CLI + copy/sign, seconds | Boxed future + copy/sign, seconds | Saving, seconds |
|---|---:|---:|---:|
| cold: baseline then candidate | 27.849 | 28.678 | -0.829 |
| cold: candidate then baseline | 28.511 | 28.603 | -0.092 |
| warm: candidate then baseline | 27.330 | 27.167 | 0.163 |
| warm: baseline then candidate | 27.209 | 27.819 | -0.610 |

Cold pairs total **0.921s slower**; warm pairs total **0.446s slower**. Neither
condition establishes useful end-to-end margin. Cargo library frontend ranges
20.56–21.30s; isolated query savings do not turn into a meaningful build saving.
All eight build/copy/sign samples completed; strict helper signature verification
passed. Native Cargo user+sys totals: cold ordinary 75.11s / boxed 75.32s;
warm ordinary 73.04s / boxed 72.57s. Peak RSS: ordinary 3.440–3.453GB,
boxed 3.409–3.411GB. These are local process-tree measurements, not hosted cost;
shared machine contention still limits attribution of sub-second differences.

Review decision: reject the candidate. The allocation/type-erasure boundary adds
maintenance and runtime cost without a measured CI benefit. Original source and
ordinary output snapshot are restored. No production code, flags, dependencies,
cache wiring, tests or installed Home changed. No full Xcode trial, runtime
acceptance, gate, hosted CI, publication or merge is claimed for this candidate.

Replay and raw evidence: `/tmp/macos-ci-frontend-bench.py`,
`/tmp/macos-ci-frontend-snapshots/results.jsonl`,
`/tmp/macos-ci-frontend-{baseline,candidate}.rs`, and
`/tmp/macos-ci-frontend{,-boxed}-profile/`. The benchmark script preserves both
source variants and restores temporary edits in `finally`; profile outputs use
`summary.txt` and `chrome_profiler.json`. Benchmark logs have the prefix
`/tmp/macos-ci-frontend-`.

Remaining blocker: frontend cost is distributed across the large library; the
largest specifically attributed small source boundary did not improve the
ordinary build. There is no measured mechanism within the current small-change
scope to advance. A further candidate needs new evidence of work that can be
removed; another unchanged overlap, backend or boxing trial is not justified.
lf-ddf2d53 remains unfinished. lf-12be237 retains ten-real-run, queue and
push-to-merged acceptance; the supplied latest real queue remains 6:24.
