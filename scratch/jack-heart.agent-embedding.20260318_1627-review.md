# Validation

- `swift test --package-path swift` ✅
- `cd swift && xcodegen generate` ✅
- `cd swift && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO -only-testing:ConcertoTests` ✅
- `cd swift && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO -only-testing:ConcertoUITests/ScreenshotPipelineTests/testCapture` ⚠️ runner crashed during bootstrap (`signal kill` before XCTest connected)
