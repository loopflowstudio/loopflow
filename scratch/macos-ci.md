# macOS CI critical path

October 13, 2026 — lf-ddf2d53. Jack Heart authorized continued implementation
and delivery. No candidate qualifies for gate or landing. Next work: select one
small frontend compilation reduction and prove a useful matched source-cold and
changed-source warm gain, including preparation and cleanup.

Rejected trials (full inputs, resource accounting and exclusions in the reviews):

- [Shell overlap](../docs/reviews/ci-under-five-minutes.md#october-13-follow-up-shell-overlap-rejected-before-publication):
  cold saved 13.453–39.668s; reversed warm pairs totaled 0.417s slower. Compiler
  overlap and cancellation passed; reliable cold/warm improvement did not.
- [Package-only codegen-units=256](../docs/reviews/ci-under-five-minutes.md#october-13-follow-up-control-cli-code-generation-trial-rejected):
  cold totaled 1.836s slower; warm saved 3.660s. Frontend took 21.49–26.56s,
  backend 3.60–4.77s. No full Xcode trial qualified.

Shared Xcode services and machine contention limit elapsed attribution. Ordinary
signed Xcode compilation and the original control CLI copy/sign path are restored.

**Delete — do not maintain:** all cuts complete: `scripts/build_xcode_ci.py`,
`python/tests/test_build_xcode_ci.py`, `_build_control_tools`,
`cmd_prepare_control_tools` and their overlap wiring are removed; the trial
`profile.dev.package.loopflow.codegen-units` override is absent from production.
Prototype checkpoint: `34c40623b19d7176a3f8e452aa2bac9c8e753013`;
raw trial data: `/tmp/macos-ci-*`.

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

Check: `git diff --check` PASS; prose-only compression, prior eight CLI build/copy/sign samples retained (performance FAIL); gate/hosted CI await a qualifying candidate.
