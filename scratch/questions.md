# Open Questions

- `xcodebuild test -project swift/LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS'` consistently fails on `ConcertoUITests/ScreenshotPipelineTests.testCapture` because the app window never appears (`XCTAssertTrue` at `swift/ConcertoUITests/ScreenshotPipelineTests.swift:21`). Is this expected locally (environment-dependent), or should this test be quarantined/fixed separately?
