# macOS CI critical path

October 13, 2026 — lf-ddf2d53. Jack Heart authorized continued implementation
and delivery. Accepted outcome: useful macOS CI critical-path margin, measured
with matched source-cold and changed-source warm inputs, including preparation
and cleanup, without material warm regression or duplicated compilation.
The supplied Task brief clarifies that every warm sample need not improve;
maintenance cost also matters. No candidate currently qualifies for gate or landing.

Rejected trials (full inputs, resource accounting and exclusions in the reviews):

- [Shell overlap](../docs/reviews/ci-under-five-minutes.md#october-13-follow-up-shell-overlap-rejected-before-publication):
  cold saved 13.453–39.668s; reversed warm pairs totaled 0.417s slower. Compiler
  overlap and scoped cancellation passed. The warm delta alone is weak evidence
  of regression; cold gains remain relevant to changed-source DerivedData misses.
  The prototype remains rejected, with cleanup complexity and hosted benefit
  unresolved; this reconciliation does not select another overlap trial.
- [Package-only codegen-units=256](../docs/reviews/ci-under-five-minutes.md#october-13-follow-up-control-cli-code-generation-trial-rejected):
  cold totaled 1.836s slower; warm saved 3.660s. Frontend took 21.49–26.56s,
  backend 3.60–4.77s. Eight CLI build/copy/sign samples passed, but cold margin
  did not improve. No full Xcode trial qualified.

Shared Xcode services and machine contention limit elapsed attribution. Ordinary
signed Xcode compilation and the original control CLI copy/sign path are restored.

Removed prototype: `scripts/build_xcode_ci.py`,
`python/tests/test_build_xcode_ci.py`, `_build_control_tools`,
`cmd_prepare_control_tools` and their overlap wiring are removed; the trial
`profile.dev.package.loopflow.codegen-units` override is absent from production.
Prototype checkpoint: `34c40623b19d7176a3f8e452aa2bac9c8e753013`;
raw trial data: `/tmp/macos-ci-*`.

Compiler timing is complete. Remaining implementation: select one small frontend
work reduction, benchmark matched cold and changed-source warm inputs in both
orders, then establish its effect through the ordinary signed Xcode path.
No reduction is selected; Cargo phase boundaries identify the dominant phase,
not the expensive source construct. The local 21.49–26.56s frontend range does
not explain all of the hosted 93-second control CLI segment.

Upstream #1545 is integrated at `414d4dd27`: selective assertion cleanup and
installation-image acquisition repair. It changes neither the control CLI build
nor Xcode ordering, and supplies no measured macOS saving. Its test deletions
remain outside this follow-up's scope.

Preserve ordinary Xcode compile proof, full Swift tests, installation/network
boundaries, copy/sign behavior, exact source/target, development provenance and
validation-only migrations. Preparation must match Xcode's SDK, deployment and
shell toolchain. Any concurrency must propagate both process failures and leave
no orphaned compiler; process groups alone failed to reap Xcode's compilers.
No broad audits, paid runners, test removal, relaxed cache identity or arbitrary
retries.

A qualifying change still needs focused checks, gate once, full hosted CI and
verified delivery before follow-up completion. [Remaining CI proof](ci-remaining-proof.md)
tracks lf-12be237's independent five-minute, ten-real-run goal and separate queue
and push-to-merged proof. No numeric acceptance is waived.

Check: `git diff --check` PASS; prose-only reconciliation reuses recorded trials; gate/hosted CI await a qualifying candidate.
