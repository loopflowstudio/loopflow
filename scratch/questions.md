# Open questions

- `xcodebuild test -project swift/LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO` still fails locally because `ConcertoUITests-Runner` is killed before establishing the UI-test connection. After the gate fix, the built Concerto app itself launches cleanly from DerivedData, so the remaining issue appears to be in the UI-test runner/bootstrap path rather than the app's framework lookup.
