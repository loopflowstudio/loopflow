# macOS CI critical path

October 13, 2026 — lf-ddf2d53. Jack Heart authorized continued CI implementation
and delivery. The shell-overlap prototype is rejected under the accepted matched
cold/warm criterion; no candidate remains for gate or landing.

## Outcome and remaining work

Required CI should finish within five minutes on real code changes. Supplied
latest queue 38452715134 still took 6:24 to aggregate completion, with Xcode 6:06
and Swift 5:29. #1543 is merged. lf-12be237 retains the independent ten-real-run,
queue and push-to-merged demonstration; this follow-up is not complete.

The prototype improved both matched source-cold pairs but failed to establish
warm improvement. Native CPU measurements cannot assign all timing variation to
overlap. The durable [review](../docs/reviews/ci-under-five-minutes.md#october-13-follow-up-shell-overlap-rejected-before-publication)
contains exact inputs, timings, resources, contrary evidence and cleanup findings.

| Matched sample order | Baseline seconds | Overlap seconds |
|---|---:|---:|
| Source-cold baseline → overlap | 85.311 | 45.643 |
| Source-cold overlap → baseline | 60.298 | 46.845 |
| Changed-source warm overlap → baseline | 33.841 | 30.868 |
| Changed-source warm baseline → overlap | 36.835 | 40.225 |

Combined warm overlap is 0.417s slower. Cold gains are 13.453–39.668s, with marked
order sensitivity. These are paired local builds with controlled temporary
source edits, not ten real PRs or a hosted gain. Further broad audit, paid runners,
test removal, cache-identity relaxation and arbitrary retries remain excluded.

Remaining blocker: a reliable macOS critical-path reduction that earns its cost
on changed-source cold and warm builds. No delivery candidate remains on these
measurements. Full hosted CI and queue acceptance remain future
work after a mechanism meets the local criterion. No Task completion is requested.

## Findings that survive the rejection

- The app CLI's authoritative command remains `cargo build --locked --bin lf
  --target-dir <checkout>/target/dev-app-control`, at repository root, with
  development provenance and validation-only migration authority. The app
  continues to run Cargo before copying/signing; neither a cache hit nor a
  parallel preparer grants permission to skip that check.
- Preparation must match Xcode's selected SDK, deployment and existing shell
  toolchain path. The first prototype selected Homebrew 1.98.1 rather than
  Xcode's rustup 1.99.0 and rebuilt dependencies. Matching effective inputs
  produced Fresh output in both invocations; every timed pair compiled the
  workspace once.
- Real process sampling proved Rust/Swift compile overlap. Reuse and overlap
  alone did not prove the accepted elapsed outcome.
- Process-group cleanup alone is insufficient: Xcode's shared service left a
  running Swift compiler after xcodebuild exited. A scoped reaper for newly
  started compilers writing this exact DerivedData path passed real cancellation
  and a fixture preserving pre-existing work/other outputs. That extra ownership
  machinery remains a cost of this approach, not a shipped capability.
- The earlier Xcode aggregate-target experiment also remains rejected. Neither
  that attempt nor warm reruns establish the original numeric goal.

Delete — do not maintain: the prototype's `scripts/build_xcode_ci.py`, its
exclusive `python/tests/test_build_xcode_ci.py`, preparation entry point/shared
helper extraction and wrapper workflow/docs consumer are removed together.
Original builds, tests, copy/sign behavior, authority, caches and job conditions
are restored. Prototype evidence is preserved in local commit
`34c40623b19d7176a3f8e452aa2bac9c8e753013`; raw data uses `/tmp/macos-ci-*`.

Check: eight focused prototype lifecycle tests PASS and real Rust/Swift
cancellation PASS; matched cold/warm acceptance FAIL (reverse warm 40.225s versus
baseline 36.835s); prototype removed, so gate/hosted delivery are not applicable.
