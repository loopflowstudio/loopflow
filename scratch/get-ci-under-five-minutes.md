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
