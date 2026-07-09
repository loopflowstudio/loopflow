# Questions / Blockers

- Local Loopflow UI validation is blocked by the Xcode UI runner in this headless gate environment. `xcodebuild test -project LoopflowSwift.xcodeproj -scheme LoopflowMac -destination 'platform=macOS' ...` reports 304 passing app tests, then fails because `LoopflowUITests-Runner` hangs before establishing a connection. A fresh DerivedData rerun reproduced the same runner hang.
