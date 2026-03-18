# Open questions

- `xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' ...` built the app and unit suites, but `ConcertoUITests-Runner` hung before establishing connection in this headless/no-rendering environment. I treated this as an environment-specific validation gap rather than a proven branch regression.
