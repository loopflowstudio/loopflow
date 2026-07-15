# QA findings

## Blocking issues

None.

The review found one blocker before the final gate: a missing journal could replace the idle start control with the history notice. The idle state now keeps the start action visible and presents the history condition alongside it.

## Polish items

- Typed Task, Project, PR, commit, and file references remain plain text in this first serial PR. Inline links and popovers belong in the next W2-174 slice.
- Hosted UI behavior was not executed because this run has no rendering host. The app and UI-test runners compile successfully with `xcodebuild build-for-testing`.

## Test results

- Python: 59 passed.
- Rust formatting and clippy: passed with warnings denied.
- Rust nextest in an isolated environment: 1,332 passed, 3 skipped. The standard gate inherits the active Loopflow task lease and fails three unrelated PR/land fixtures; clearing the ambient `LF_TASK_*` and control variables makes all 1,332 tests pass.
- Website: 59 passed, 3 skipped.
- Swift: 114 passed across 21 suites; multiplatform boundary checks passed.
- macOS app and UI-test runner compile: passed.
- End-to-end smoke: passed.
- Current Product journal: returned a `partial` snapshot with the latest 12 readable turns and `truncated: true`; the incompatible tail remained untouched.
- Process-level history query: 20 reads in 3.08 seconds, about 154 ms per invocation.

## Review ritual

The durable journal remains the only conversation source of truth. The CLI snapshot is a bounded, read-only projection; the Mac app paints that projection before endpoint discovery, then reconciles replay and live SSE frames by stable turn id. Missing, partial, and unavailable evidence stay distinct. Failure roll-ups hide only exact-equivalent operational retries and retain every raw turn in the journal and in the disclosure details.
