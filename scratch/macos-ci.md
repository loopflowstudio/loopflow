# macOS CI critical path

October 13, 2026 — lf-ddf2d53. Jack Heart authorized continued implementation
and delivery. Accepted outcome: useful macOS CI critical-path margin, measured
with matched source-cold and changed-source warm inputs, including preparation
and cleanup, without material warm regression or duplicated compilation.
The supplied Task brief clarifies that every warm sample need not improve;
maintenance cost also matters. No candidate currently qualifies for gate or landing.

Rejected trials; linked reviews retain inputs, resource accounting and exclusions:

| Trial | Source-cold result | Changed-source warm result |
|---|---|---|
| [Shell overlap](../docs/reviews/ci-under-five-minutes.md#october-13-follow-up-shell-overlap-rejected-before-publication) | 13.453–39.668s saved per pair | 0.417s slower combined |
| [Package-only codegen-units=256](../docs/reviews/ci-under-five-minutes.md#october-13-follow-up-control-cli-code-generation-trial-rejected) | 1.836s slower combined | 3.660s saved combined |
| [Boxed planning future](../docs/reviews/ci-under-five-minutes.md#october-13-follow-up-nested-planning-future-attribution) | 0.921s slower combined | 0.446s slower combined |

Overlap and scoped cancellation passed. Its warm delta is weak regression evidence;
cold gains remain relevant to changed-source DerivedData misses, but cleanup
complexity and hosted benefit remain unresolved. This pass selects no new trial.
Shared Xcode services and machine contention limit elapsed attribution.

October 13 attribution is complete: 17,898 type-check roots and 16,159 borrow-check
queries distribute the library's cost; no single multi-second source function
was identified. Frontend cost dominated codegen (21.49–26.56s versus 3.60–4.77s).
Boxing `sync_repository` reduced caller queries from 208/224ms to 4.5/1.4ms
without reducing ordinary build time. Further implementation needs new evidence
of removable work before matched cold/warm and ordinary signed Xcode proof.
Local frontend measurements still do not explain the hosted 93-second CLI segment.

Delete — do not maintain (all removed): `scripts/build_xcode_ci.py`,
`python/tests/test_build_xcode_ci.py`, `_build_control_tools`,
`cmd_prepare_control_tools`, their overlap wiring, the
`profile.dev.package.loopflow.codegen-units` override and boxed `sync_repository`.
Ordinary signed Xcode and CLI copy/sign paths are restored. Preserve the single
async sync function, await ordering, timeouts, cancellation and acquisition tests.
Prototype checkpoint: `34c40623b19d7176a3f8e452aa2bac9c8e753013`;
raw trial data: `/tmp/macos-ci-*`.

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

Check: reuse eight matched `cargo build --locked --bin lf` + copy/sign samples and `codesign --verify --strict` PASS (performance rejected); `git diff --check` PASS; `lf context` and direct cl100k_base memory count within limits; prose-only compression needs no build/test rerun; gate/CI await a qualifying change.
