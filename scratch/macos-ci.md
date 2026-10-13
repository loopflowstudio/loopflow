# macOS CI critical path

October 13, 2026 — lf-ddf2d53. Jack Heart authorized continued CI implementation
and delivery. The shell-overlap prototype is rejected under the accepted matched
cold/warm criterion; no candidate remains for gate or landing.

October 13 targeted iteration rejected package-only codegen-units=256. Matched
serial CLI/copy/sign cold pairs totaled 1.836s slower; changed-source warm pairs
saved 3.660s. Frontend compilation dominates (21.49–26.56s), backend 3.60–4.77s.
No full Xcode trial qualifies. [Compiler review](../docs/reviews/ci-under-five-minutes.md#october-13-follow-up-control-cli-code-generation-trial-rejected)
owns reproducible inputs, phase timings, resource accounting and rejection.
Delete — do not maintain: rejected trial profile override; absent from production.
Next unresolved candidate: a small frontend work reduction; no mechanism accepted.

Remaining implementation: a small macOS critical-path reduction with a reliable
matched cold/warm gain; no replacement mechanism is accepted. A qualifying
candidate still needs both process failures propagated, orphan-free cancellation,
focused checks, gate once, full hosted CI and verified delivery. None of those
later acceptance stages is established by the rejected prototype. Preserve ordinary
Xcode compile proof, full Swift tests, installation/network boundaries, copy/sign
behavior, exact source/target, development provenance and validation-only migration
authority. Broad audits, paid runners, test removal, relaxed cache identity and
arbitrary retries remain excluded. Follow-up completion requires its measured
accepted outcome and verified merge; the five-minute, ten-real-run goal remains
with lf-12be237, including separate queue and push-to-merged proof.

Cold overlap saved 13.453–39.668s, but warm results reversed with order and totaled
0.417s slower. Actual Rust/Swift overlap and cancellation passed; the required
cold/warm gain did not. Shared Xcode services and contention limit attribution.
The durable [review](../docs/reviews/ci-under-five-minutes.md#october-13-follow-up-shell-overlap-rejected-before-publication)
owns matched timings, inputs, resource accounting, exclusions and cleanup findings.
[Remaining CI proof](ci-remaining-proof.md) owns hosted timings and lf-12be237's
independent ten-real-run, queue and push-to-merged obligation.

Existing behavior: ordinary signed Xcode compilation and the original control CLI
copy/sign path are restored; the overlap wrapper, exclusive tests and preparation
entry point are absent. No deletion targets remain.
Prototype: `34c40623b19d7176a3f8e452aa2bac9c8e753013`;
raw data: `/tmp/macos-ci-*`. Any later preparation must match Xcode's SDK,
deployment and shell toolchain; process groups alone did not reap its compilers.

Check: eight matched CLI builds/copy/sign samples PASS, performance FAIL; strict helper signature and `git diff --check` PASS; gate/hosted CI deferred until a qualifying candidate.
