# Questions

## Open validation blocker

- On March 20, 2026, `cd swift && xcodegen generate && xcodebuild clean test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO` still failed after the package/unit suites passed because `ConcertoUITests-Runner` exited during bootstrap (`Early unexpected exit, operation never finished bootstrapping`; underlying message: `Test crashed with signal kill before establishing connection`).
