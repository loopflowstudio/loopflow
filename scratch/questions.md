# Questions

## Open validation blocker

- On March 20, 2026, `cd swift && xcodegen generate && xcodebuild clean test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO` still failed after the package/unit suites passed because `ConcertoUITests-Runner` exited during bootstrap (`Early unexpected exit, operation never finished bootstrapping`; underlying message: `Test crashed with signal kill before establishing connection`).
- A non-clean `xcodebuild test` run first hit a stale DerivedData linker write error for `ConcertoUITests`, but the clean rerun above reproduced the longer-standing bootstrap failure.

## Journal escalation signaling

- The new journal protocol supports `*.escalated` events end-to-end, but the CLI currently has no dedicated escalation error/signal type to distinguish escalation from ordinary failure. This implementation emits `run/flow/step.errored` for current `anyhow::Error` paths and leaves `*.escalated` available for future callers that can provide an explicit signal.
