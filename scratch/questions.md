# Open Questions / Blockers

- `xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO` fails locally because `ConcertoUITests-Runner` exits early (`signal kill`) before bootstrapping. Unit tests and Swift package tests pass.
