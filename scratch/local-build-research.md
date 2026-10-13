# Local build research · October 13, 2026

Research for lf-8f870b1, following Jack Heart's autonomous delivery request.
This records baseline research before implementation; the design and storage
review own the subsequent matched CLI gain and disposable reclamation evidence.

## Baseline owners and boundaries

- `Cargo.toml`: dev/test already have debug=1, incremental=false and targeted
  SHA-256/SQLite optimization. A profile change needs measured tradeoffs.
- `scripts/test.py::_rust_commands`: inventory, fmt, clippy, then materialized
  Cargo/nextest tests with four jobs and external network denial. Unlike hosted
  CI (`.github/workflows/ci.yml`), it does not supply `LOOPFLOW_TEST_LF`.
- `rust/loopflow-test-support/src/lib.rs::lf_binary`: one CLI resolution per
  test process, using that supplied candidate or nested `cargo build --offline
  --bin lf`. Nextest's independent processes repeat resolution. Homes remain
  independent. Avoiding nested Cargo is an existing supported capability.
- Test builds include axum and Tokio test-util dev dependencies; ordinary CLI
  builds have a different feature graph. Ordinary CLI preparation below compiled
  Tokio/Hyper/Reqwest variants even after integration preparation. Source
  restoration also invalidated the root package: the whole refresh time cannot
  be attributed to feature differences. Sharing a directory does not itself
  remove duplicate compilation.
- `scripts/materialize_rust_tests.py`: copies exact tracked/untracked inputs to
  a fresh disposable checkout, materializes draft migrations, removes ambient
  work authority, and defaults to the source checkout's target directory. The
  temporary source root and schema frontier differ from ordinary source builds.
  TESTING.md explicitly forbids overlapping source builds in that target.
- `scripts/loopflow-dev.py`: build runs SwiftPM only. Installation and Xcode's
  bundling script additionally build a CLI in `target/dev-app-control`, explicitly
  development/validation-only. These match ordinary `build.rs` defaults, but
  external overrides, source identity and materialized migrations still matter.
  No installation or real Home mutation was exercised.
- `scripts/resource_envelope.py`: already inventories and recovers build roots
  (`target`, `.build`, `swift/.build`, `website/node_modules`). Eligibility needs
  inactivity and 24 hours without any filesystem modification, plus >24 GiB
  size or disk pressure below 64 GiB free. Recovery is limited to eight roots /
  64 GiB and a nonblocking host cleaner lock. Verification stops below 32 GiB.
  This pressure policy does not give all completed outputs an age bound.
- `lf wt prune` already knows terminal PR evidence, dirty/current/persistent
  worktrees and unfinished Task protection. Owners: `git/worktrees.rs`,
  `lf/commands/ops/mod.rs::protected_worktree_paths`, `git/mod.rs` leases,
  `store/sqlite.rs::read_nonterminal_task_worktrees`. The latter reads production
  ownership without migrations; it is not a terminal build list.
- `ops/task/lifecycle.rs::cleanup_completed_task` removes a resolved merged-PR
  checkout only with idle execution. PR-less, unresolved, primary, dirty or
  active work can remain. Build reclamation should preserve those checkouts.

## Refreshed inventory

Read-only resource receipt: 83 registered checkouts, 17 with output,
**141.96 GiB allocated** (71.65 GiB marked active, 70.30 GiB marked inactive),
**69.13 GiB free**. Activity is the current cwd detector's answer, not Task
completion or deletion permission. One 25.55 GiB root exceeded its threshold.
No real sibling output was deleted. The CI checkout now has 7.65 GiB across
all measured build roots; the handoff's earlier target-only category differs.
This checkout started without Rust or Swift output.

## Baseline

HEAD: `f2855f7bd46aa93d57b3a2e56fc8ec917f266ea3`.
Cargo.lock SHA-256:
`a1f0adb56555425151764a415aeb536e986116b81c84d0ed779704b89b3c92e2`.
Rust 1.98.1 Homebrew; Swift 6.2.3; arm64 macOS. Four compiler jobs;
inherited LF_* removed; closed stdin. Commands sequential, other host workloads
active. Single samples establish baselines, not causal before/after improvements.
Cold means empty local build output, not cold registry/download or OS caches.

| Command / work | Cold preparation | Unchanged preparation | Isolated execution |
| --- | ---: | ---: | ---: |
| `cargo test -p loopflow --lib build_info::tests:: --no-run` | 83.195s | 0.132s | Six tests, 2.499s including wrapper (0.91s harness) |
| `cargo test -p loopflow --test integration agent_startup_tests:: --no-run`, after library build | 35.425s additional | 0.130s | One fixture, 4.944s including wrapper (3.87s harness) |
| `swift build --package-path swift --jobs 4` | 34.788s | 0.463s | Build only |
| `loopflow-dev.py bundle-control-tools --output <disposable bundle>` | 66.289s | 0.261s | Build/copy only |
| Small edit in `src/build_info.rs`, library compile | 45.606s | — | Six tests, 2.394s including wrapper |

The small edit appended a temporary comment and restored original bytes in
`finally`. It measures module invalidation, not a semantic feature. Swift and
control tools are separate components, not an installed app/Xcode end-to-end
measurement. Test execution used `scripts/test_network.py` and disposable Homes.

| Output stage | Logical file bytes | Allocated file bytes |
| --- | ---: | ---: |
| Rust after cold library preparation | 1,576,196,887 | 1,602,641,920 |
| Rust after integration preparation | 2,926,304,752 | 2,970,345,472 |
| Rust including separate app-control build | 4,867,155,706 | 4,938,203,136 |
| Swift after cold build | 838,534,056 | 855,986,176 |

Control output alone: 1.83 GiB (`du`); Swift: 0.80 GiB. These are allocation
counts, not proven exclusive APFS physical blocks. No reclamation claim.

Real CLI-heavy library fixture:
`local_planning::public_local_task_places_and_runs_without_a_planning_provider`.
After restoring source, ordinary CLI preparation took **34.612s**, including root
and different dependency variants. With that CLI warm, nested Cargo resolution
plus fixture took **5.837s**; supplying the exact candidate through
`LOOPFLOW_TEST_LF` took **3.402s**. Both selected/passed one test and exercised
offline Task creation, checkout and Flow execution. This ordered pair suggests
value but does not prove a repeatable 2.435s gain or isolate filesystem warming.
Two initial fixture attempts used an incorrect module prefix and selected zero
tests; their 0.06s results are excluded.

After ordinary CLI preparation, `du -sk` measured target 5,948,464 KiB,
source debug 4,026,720 KiB, control target 1,921,736 KiB and Swift 836,232 KiB.
A lib-only cold footprint understates the complete developer workflow.

Raw native-time CPU/RSS logs, exact commands and JSON rows:
`.lf/tmp/local-build-baseline/`. Inventory:
`/tmp/local-build-resource-before.json`. This note carries findings into the
next step; raw logs stay outside recursive scratch context. Final matched results
belong in durable docs before delivery clears scratch.

## Preservation counterexample

Python `_running_cwds` and Rust `running_workspace_paths` inspect cwd only.
A disposable C executable under target, launched with cwd `/tmp`, was still
running while Python marked its root inactive. `lsof -p <pid> -Fn` showed the
executable handle. `activity-proof.json` records the result. Child and temporary
output were removed; cleanup never ran. Cwd and old mtimes cannot prove a root
safe to reclaim. Python retains all builds on activity-reader failure; Rust's
cwd reader returns an empty set on failure. Task protection is separate.
The cleaner lock serializes cleaners, not compiler writers; Python does not
recheck process activity immediately before `_remove_build_artifacts` deletion.

## Native writer exclusion · October 13

- Cargo documents profile-local locks, not a target-root lock, and artifact JSON
  carries executable paths. See [layout source](https://doc.rust-lang.org/stable/nightly-rustc/src/cargo/compiler/layout.rs.html)
  and [artifact protocol](https://doc.rust-lang.org/cargo/reference/external-tools.html).
  Held lock inodes must survive deletion: unlinking a lock allows replacement
  writers. Native behavior must be proved against the actual toolchain.
- October 13 implementation probe on Cargo 1.98.1: an exclusive flock on
  `target/debug/.cargo-lock` blocks `cargo build --offline --jobs 4`, but
  `cargo clean --offline` succeeds and unlinks that file. A subsequent build
  succeeds and recreates the executable while the original descriptor remains
  exclusively locked. Its replacement lock has a different inode. The fixture
  was a disposable dependency-free package; no repository output was cleaned.
  Receipt: `.lf/tmp/local-build-baseline/cargo-clean-lock-proof.json`.
  This is an ordinary native command, not a hypothetical writer ignoring locks.
- SwiftPM's .build/.lock contains a PID, not the native exclusion lock. Disposable
  warm builds ignored flock/lockf on that file and flock on the directory.
  The [SwiftPM 6.2 command owner](https://github.com/swiftlang/swift-package-manager/blob/swift-6.2-RELEASE/Sources/CoreCommands/SwiftCommandState.swift)
  uses [TSC FileLock.prepareLock](https://github.com/swiftlang/swift-tools-support-core/blob/main/Sources/TSCBasic/Lock.swift):
  a canonical-path-derived lock in the native temporary directory. Holding that
  lock blocked the real build for three seconds, emitted the wait message, and
  releasing it let the build pass. All package/output fixtures were disposable.

## Design handoff

The [design](local-build-speed-space.md) owns selected mechanisms, exclusions,
coordination and acceptance. Earlier kickoff candidates remain at
`4db478adee7ee0b1c29c7944b6a9acd3657187eb:scratch/local-build-research.md`.
Jack Heart subsequently accepted practical cooperating-lease coordination on
October 13. The Cargo lock-replacement receipt remains contrary evidence against
universal native exclusion; the design owns the accepted cleanup policy.

Research checks: six build-info tests, one startup fixture and one local-planning
fixture pass under network isolation; Rust, Swift and control cold/warm builds
pass; temporary source edit restored. Matched improvements, cleanup/reclamation,
profile selection and full CI remain with later Flow steps.
