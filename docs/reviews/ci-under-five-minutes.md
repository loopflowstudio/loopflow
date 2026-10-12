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
