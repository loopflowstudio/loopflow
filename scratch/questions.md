# Open questions

- Can `ConcertoUITests/ScreenshotPipelineTests.testCapture` be stabilized in this environment so full `xcodebuild test -scheme Concerto` passes reliably?
- The installed lfd launchd plist clobbers session tokens during dev. Should `scripts/dev.py lfd` handle this automatically (bootout + plist rename)?
