# Open questions

- `xcodebuild test -project swift/LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO` still fails locally because `ConcertoUITests-Runner` is killed before establishing the UI-test connection. On March 19, 2026, a clean DerivedData rebuild still reproduced that bootstrap kill after the app and unit/package coverage in the run passed.
