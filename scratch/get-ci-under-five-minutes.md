# CI under five minutes

Jack Heart requested measurement and a proposed plan before implementation; CI is the objective, local speed is incidental. No approach or paid runner has been selected.

Baseline and step breakdown: `docs/reviews/ci-under-five-minutes.md`. Raw GitHub jobs: `scratch/ci-baseline/`. Seven full successful runs show three independent blockers: Rust 18:22–20:40, disposable installation 7:43–10:46, Swift 5:46–7:32.

Proposed independently deliverable Tasks, pending direction review:
1. Reduce Rust CI execution through fixture-cost investigation, then balanced shards sharing a compilation if still necessary; retain network isolation and materialized migrations.
2. Reuse builds across disposable installation scenarios and parallelize independent proofs without losing OS-account isolation or published provenance coverage.
3. Remove serial macOS discovery compilation from the Swift path; preserve native platform and headless Desktop coverage.

This Task retains baseline, runner comparison and ten-consecutive-run acceptance. Confirm whether “blackforge” means Blacksmith before evaluating it. No new recurring cost may be adopted without Jack's decision. No tests have been selected for deletion. Queue proof reuse remains a separate design question, not a safe assumption.

Checks: read-only GitHub API/log analysis completed; no production code or workflow changes, so no build/test rerun. Review identified three independent critical paths, preventing a Rust-only plan from falsely claiming completion.

## Jack Heart's direction — 2026-10-12

Jack confirmed the runner platform is Blacksmith. He requested a subagent audit comparing behaviors and functionality described on the website and in documentation, plus evidence of extensive internal use, with actual test coverage to identify mismatches. This contribution is read-only except for its report; implementation and deletion recommendations await review.

Jack Heart agreed to invest heavily in research and measurement before implementation. Prioritize evidence gathering and the behavior-to-proof audit before selecting optimizations or removing tests. The proposed three-way split is provisional, not authorization to begin those implementations.

Jack Heart clarified the optimization objective: reduce work first and use the most cost-efficient hardware for the remaining work. Both elapsed time and total CI cost matter; premium hardware solely for speed is not the intended strategy. Evaluate Blacksmith against the current platform on useful work per cost, including repeated compilation, runner minutes, cache/artifact transfer and retries. Parallelization must account for total work and cost, not just wall-clock improvement. Paid-platform adoption still requires a concrete cost comparison and Jack's decision.

Jack Heart redirected the billing investigation out of scope; Loopflow CI elapsed time is the focus. The behavior audit is complete in `scratch/testing-audit.md`. Current research priority is to measure nested Cargo invocations and build invalidation before selecting a repair. The demonstrated under-five-minute goal remains active; no speedup has yet been measured.

## Rebuild cause — measured locally

A dependency-free Cargo reproduction confirmed that using the same environment name for a build-script input (`rerun-if-env-changed`) and compiled output (`rustc-env`) makes nested Cargo invalidate the build: omitted input becomes exported default in the test process, then the outer command changes it back. Logs: `scratch/ci-baseline/cargo-reentry-probe.log`.

The real Loopflow build's fingerprint trace confirms `LOOPFLOW_BUILD_PROVENANCE` changes from `None` to `Some("development")` when entering the nested-equivalent CLI build after preparing library tests. `LOOPFLOW_MIGRATION_AUTHORITY` uses the same input/output pattern. Evidence: `scratch/ci-baseline/provenance-rebuild.log`.

Proposed minimal repair: give compiled provenance/authority outputs distinct names from externally supplied build inputs in build.rs and build_info.rs. Preserve input names, explicit release/published values, and source/version identity. Do not normalize away an actual provenance change. Pair with a prepared CLI artifact supplied to library fixtures so nextest does not invoke Cargo per process; preserve standalone focused cargo-test behavior and custom target directories. These are distinct questions: eliminating invalidation does not eliminate repeated Cargo metadata/lock work.

Jack Heart also values reducing excess test code independently of CI time savings. He explicitly retains genuinely useful tests. Review deletion candidates by the meaningful regression that would escape without them, considering equivalent remaining coverage. Prose/implementation freezing and duplicate proof are candidates; unique behavioral protection stays. Test reduction is a separate desired outcome, not merely a timing tactic.

First repair: build.rs now emits `LOOPFLOW_COMPILED_PROVENANCE` and `LOOPFLOW_COMPILED_MIGRATION_AUTHORITY`; build_info.rs consumes those outputs. External build inputs retain their original names. Delete list: the two input/output name collisions, with no compatibility aliases. Warm real-build comparison: before, exported-input build 0.27s and returning to omitted inputs 42.57s; after, nested-output environment 0.16s and outer environment 0.13s (initial recompilation 38.80s excluded). This isolates a local invalidation cost; hosted total savings remain unproved.

Test-reduction candidate review: the first three builtins.rs tests contain about 40 lines freezing Wave/metric prose and stale Linear-roadmap wording. Removing these checks permits editorial change; it does not remove a runtime proof of planning or metric ownership because they never execute those behaviors. Preserve catalog discovery and retired `ingest` absence in the existing catalog test rather than deleting that distinct assertion. Keep release-state preservation guidance checks pending stronger review of their safety purpose. Do not delete the resolution matrix until its actual dispatch obligations have a stronger, smaller replacement. File independent test-code reduction separately from the build repair.

Context inspection: scratch remains below both limits. Existing Wave memory is 17,158 tokens against 16,000; broad historical-memory curation is outside the CI repair and remains deferred.

Jack Heart requested real PR test-failure history over roughly the last month. Rank recurring failures, then distinguish useful regression detection from flakes, fixture/infrastructure failures and unnecessary maintenance. Failure frequency alone grants no deletion: correlate reruns, changed commits and actual assertion/fix evidence. Report sample coverage, unknown classifications and censored/missing logs.

Checks: network-isolated build_info tests 6/6 pass with default inputs and 6/6 with explicit release/published inputs; emitted compiled fields read release/published; authored Flow Session fixture 1/1 passes; cargo fmt --all -- --check and git diff --check pass. Clippy is in progress. Local cache re-entry experiment removes the 42.57s unnecessary warm rebuild; hosted performance is still unmeasured.

Checks: cargo clippy --all-targets -- -D warnings passes (44.31s); fmt and focused proofs above remain valid. Review: only compiled output names change; explicit external inputs and authority boundaries remain intact. No CI tests or jobs removed. Hosted acceptance remains outstanding.
