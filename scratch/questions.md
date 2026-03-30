# Open questions

- `xcodebuild test -project swift/LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO` still fails locally after a clean DerivedData wipe because `ConcertoUITests-Runner` exits before bootstrapping (`signal kill`). Swift package tests pass, so this looks like an environment/UI-test-runner problem rather than a compile failure, but it remains the main pre-merge risk to confirm in CI.
