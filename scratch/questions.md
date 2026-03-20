# Questions

## Open validation blocker

- `xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO` failed twice on March 20, 2026 because `ConcertoUITests-Runner` was killed before it finished bootstrapping (`Early unexpected exit, operation never finished bootstrapping`). Pre-existing issue, not caused by this branch. Swift package tests and all non-UI validation passed.

## Journal escalation signaling

- The new journal protocol supports `*.escalated` events end-to-end, but the CLI currently has no dedicated escalation error/signal type to distinguish escalation from ordinary failure. This implementation emits `run/flow/step.errored` for current `anyhow::Error` paths and leaves `*.escalated` available for future callers that can provide an explicit signal.
