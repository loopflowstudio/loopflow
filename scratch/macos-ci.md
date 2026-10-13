# macOS CI critical path

October 13, 2026 — lf-ddf2d53. Jack Heart accepted continued CI implementation
and delivery. The overlap mechanism below is selected for measurement, not yet
accepted as a performance improvement.

## Problem and observable outcome

Developers should receive required CI results within five minutes on real code
changes. Latest queue run 38452715134 took 6:24 to aggregate completion (6:25
updatedAt proxy): Xcode finished at 6:06, Swift at 5:29. Its Xcode step took
271s, including 93s of bundled CLI compilation. Dependency caches hit while
changed Swift inputs missed DerivedData. [Remaining proof](ci-remaining-proof.md)
retains the recent runs; [durable evidence](../docs/reviews/ci-under-five-minutes.md)
records merged #1543 and the rejected aggregate-target experiment.

This infrastructure-only slice demonstrates the win through headless build
timestamps and required Actions jobs: ordinary app/test-runner compilation and
CLI preparation overlap, total elapsed work falls, and the same signed current
CLI reaches the app. No display-session demo is required. A faster local build
alone does not establish hosted improvement. lf-12be237 retains ten consecutive
typical-code PR runs, queue timing and push-to-merged acceptance independently.

## Current architecture and findings

`loopflow-ui-test` restores `target/dev-app-control` dependencies and exact-source
DerivedData, generates the ordinary project, then runs signed `build-for-testing`.
`swift/project.yml` invokes `loopflow-dev.py bundle-control-tools` after app
compilation. `_copy_bundled_tools` runs Cargo, copies `debug/lf`, then Xcode's
script signs the helper before app signing. Test bundles compile afterwards.

The authoritative build inputs remain the checkout, Cargo.lock, toolchain,
profiles and compiled development/validation-only authority. Cargo owns reuse;
the app owns its copy/sign step. Actions job/step completion timestamps own
hosted timing; log order and workflow updatedAt are weaker proxies.

Confirmed October 13:

- The exact CLI command is `cargo build --locked --bin lf --target-dir
  <checkout>/target/dev-app-control`, run at repository root with
  `LOOPFLOW_BUILD_PROVENANCE=development` and
  `LOOPFLOW_MIGRATION_AUTHORITY=validation_only`. CI dev/test debug levels are
  zero. It cannot reuse the Swift transport or published CLI's authority target.
- A disposable dependency-free Cargo probe with a two-second build script ran
  two concurrent identical builds. The second reported a target lock wait and
  `Fresh`; both succeeded in 2.60s total. This establishes serialization/reuse
  with Cargo 1.98.1, not actual Loopflow environment equivalence or elapsed gain.
  [Cargo's build cache contract](https://doc.rust-lang.org/stable/cargo/reference/build-cache.html)
  assigns outputs to the selected target directory.
- The post-build script intentionally always runs; retaining it preserves source
  verification and helper signing even with restored DerivedData. No Xcode graph
  change or cache-key relaxation is needed. XcodeGen generation succeeds.
- Xcode supplies SDK/deployment settings to its shell phase. An earlier Cargo
  build outside Xcode must use matching effective inputs; real fingerprint logs
  must prove the post-build command does not recompile. Identical argv alone
  does not prove this. `xcodebuild -showBuildSettings` confirms deployment 15.0
  and the selected macOS26.2 SDK here; resolve the selected SDK with `xcrun`
  rather than hardcoding its path/version.
- Existing `scripts/test.py` uses dedicated process groups for owned children.
  Waiting for a parent alone cannot prove compiler descendants exited. Shared
  Xcode services are not safe targets for machine-wide name-based killing.

## Chosen approach and alternatives

Extract the existing Cargo build into one private function in `loopflow-dev.py`,
returning its output path. Add `prepare-control-tools` using that function;
`bundle-control-tools` continues to call it before copying. This leaves one build
definition and ordinary local behavior intact.

Use a small CI-specific `scripts/build_xcode_ci.py` for this one command pair.
Start preparation and the unchanged Xcode command in separate owned process
groups, in the same checkout after existing restores/project generation. Pass
matching selected SDK/deployment settings to preparation without changing the
ordinary app command's semantics. Retain native timing and emit flushed start,
finish and total timestamps. The Xcode post-build invocation either waits on
Cargo's native lock or sees a fresh build; it still copies and signs.

Both exit statuses are required. On either failure, launch error, SIGTERM or
interrupt, terminate the remaining owned groups, allow bounded graceful exit,
kill surviving group members, and reap direct children. Cleanup must still
handle descendants after their leader exits. Preserve diagnostic output and
the original failing status; success requires both processes complete. Keep
the existing 12-minute CI timeout. Missing artifacts and unreadable build input
remain errors, never permission to use an installed or stale CLI.

Alternatives:

- Xcode aggregate target: rejected by run 38430532823 (5:44 Actions, 5:49
  push-to-checks); logs did not establish intended overlap. Do not revive it.
- Prebuild serially: useful benchmark control, but moves rather than removes
  the same cost. It is not the target architecture.
- Cargo locks without extracting the command: risks duplicated definitions
  drifting in authority or flags. Reuse the existing build owner instead.
- Cross-job CLI sharing: adds transfers and source/provenance matching to a
  small same-job problem; not selected. Paid runners, relaxed cache identity,
  test removal and generic scheduling are excluded.

Wild success: source changes stop exposing 93s of serial Rust work while all
ordinary build proofs remain. Wild failure: CPU contention makes Xcode slower,
SDK differences trigger a second compile, or cancellation leaves compilers.
Those are rejection criteria, not follow-up polish.

## This slice and measurement

1. Implement the shared preparation entry point and one CI wrapper, plus focused
   process tests. Before changing the workflow, prove both-success, each/both
   failure, launch failure and cancellation using disposable commands with
   real descendant processes; assert status and no surviving owned descendants.
   Focused check: `uv run pytest python/tests/test_build_xcode_ci.py`.
2. Benchmark unchanged baseline and candidate with fixed source/toolchain,
   absolute checkout/target paths, signing flags and dev/test profiles. Restore
   identical snapshots at those paths before each sample. Pair cold outputs
   (same dependencies, no workspace output or DerivedData) and warm outputs
   (same dependency/build snapshot, same real source edit requiring recompilation).
   Include completely fresh dependency-cache timing separately when available;
   do not relabel dependency-warm source-cold as fully cold. Alternate order to
   expose thermal/cache drift. No `cargo clean` against other work.
3. Record preparation, Swift compile, bundle phase, test-bundle and total
   intervals, native user/system CPU and peak RSS, compiled crates, cache states,
   source/toolchain identity and failures. Count all preparation and cleanup in
   elapsed time. Confirm actual intersecting compile intervals, not buffered log
   adjacency. Post-build Cargo must report fresh current output; byte comparison
   accounts for signing and disposable runtime checks verify compiled authority.
4. Retain the candidate only when matched cold/warm evidence shows a useful
   critical-path gain with no duplicated compile work or correctness regression.
   No invented numeric waiver: five minutes remains the aggregate target.
   If results contradict overlap, remove the experiment and preserve the exact
   result; identify the next measured small reduction or the remaining blocker.
5. Wire only the existing Xcode CI step to the proved wrapper. Preserve all job
   conditions, caches, signing, full Swift execution, installation and network
   boundaries. Persist measurements/rejected options in the durable review doc
   before scratch clearing and delivery.

## Done when and gate

The same changed-source build produces the ordinary app and both test bundles,
with current development/validation-only CLI, valid helper/app signatures, no
duplicate CLI compilation and no owned compiler remaining after failure. Matched
cold/warm before/after totals and actual compile overlap demonstrate a worthwhile
gain; a full hosted candidate and queue retain all required proof. Completion of
this follow-up additionally requires verified merge and its measured outcome;
it does not complete lf-12be237's aggregate demonstration.

Gate once, headless:

- `uv run pytest python/tests/test_build_xcode_ci.py python/tests/test_ci_proof.py
  python/tests/test_release_automation.py`; process lifecycle, authority build and
  required-job behavior pass. Include a changed-source preparation-to-bundle
  scenario through the real shared build owner, using disposable output/Home.
- `actionlint .github/workflows/ci.yml` and Python formatting/static checks for
  changed files pass. No Rust source changes are planned.
- `CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 uv run python
  scripts/build_xcode_ci.py` builds the ordinary signed app/unit/UI-test runners
  with the existing exact Xcode arguments. Verify `codesign --verify --strict`
  on `.build/xcode-derived-data/Build/Products/Debug/Loopflow.app/Contents/MacOS/lf`
  and `codesign --verify --deep --strict` on the app, from `swift/`.
- `scripts/test_desktop.sh -Xswiftc -gnone` retains full headless Swift execution.
  Branch executable runtime checks use disposable Homes and existing network
  isolation. No installed Home or Applications mutation.
- Full hosted CI owns its existing matrix and unavailable local platform checks;
  report creation-to-aggregate, queue and actual push timing separately. Exclude
  scratch-only checkpoints and warmed reruns from the ten-real-run sample.

Delete — do not maintain: replace only the inline Xcode CI invocation with its
single wrapper consumer after proof. No production/test deletion is needed.
Forbid a second Cargo definition, aggregate target, skipped post-build check,
installed CLI fallback, changed authority target or suppressed child failure.

Remaining: actual environment/fingerprint proof, implementation, matched real
compile measurements, gate, full hosted CI and supported delivery in the supplied
Flow. Task execution authorization does not turn these pending checks into passes.

Check: disposable concurrent `cargo build --offline -vv` probe PASS (2.60s,
lock wait then Fresh); XcodeGen/settings PASS; real overlap/performance pending.
