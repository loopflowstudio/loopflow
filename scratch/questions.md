# Open questions

- `xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -derivedDataPath /tmp/loopflow-ui-test -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO` still ends with `ConcertoUITests-Runner ... Early unexpected exit` on this machine after all Swift unit tests pass. The failure happens before the UI test establishes its connection, so this gate leaves it documented rather than fixed. Verify on CI/macOS runner before landing.
