# Open Questions / Blockers

- `xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO` still fails locally because `ConcertoUITests-Runner` exits early (`signal kill`) before bootstrapping.
- Verification workaround: `xcodebuild test ... -skip-testing:ConcertoUITests` passes, and `swift test --package-path swift` passes.
