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
- [Boxed planning future](../docs/reviews/ci-under-five-minutes.md#october-13-follow-up-nested-planning-future-attribution):
  rustc profiling attributed expensive Flow caller queries to `sync_repository`.
  Boxing reduced those queries from 208/224ms to 4.5/1.4ms, but eight ordinary
  build/copy/sign samples totaled 0.921s slower cold and 0.446s slower warm.
  No useful end-to-end gain; original source and ordinary outputs restored.

Shared Xcode services and machine contention limit elapsed attribution. Ordinary
signed Xcode compilation and the original control CLI copy/sign path are restored.

Removed prototype: `scripts/build_xcode_ci.py`,
`python/tests/test_build_xcode_ci.py`, `_build_control_tools`,
`cmd_prepare_control_tools` and their overlap wiring are removed; the trial
`profile.dev.package.loopflow.codegen-units` override is absent from production.
Prototype checkpoint: `34c40623b19d7176a3f8e452aa2bac9c8e753013`;
raw trial data: `/tmp/macos-ci-*`.

October 13 attribution is complete: 17,898 type-check roots and 16,159 borrow-check
queries distribute the library's cost; no single multi-second source function
was identified. The small specifically attributed boundary did not reduce build
time. No measured mechanism within the current small-change scope qualifies.
Further implementation needs new evidence of removable work, before matched
cold/warm and ordinary signed Xcode proof. The local frontend measurements still
do not explain all of the hosted 93-second control CLI segment.

Delete — do not maintain: rejected overlap wiring, codegen override and boxed
`sync_repository` trial are absent. Preserve the original single sync function,
await ordering, timeouts, cancellation and on-demand acquisition tests.

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

Check: eight matched `cargo build --locked --bin lf` + copy/sign samples and `codesign --verify --strict` PASS; performance rejected; `git diff --check` PASS; no code remains to gate or publish.
