# Open questions

- `xcodebuild test -only-testing:ConcertoUITests/ScreenshotPipelineTests/testCapture` still crashes the `ConcertoUITests-Runner` on this host before XCTest connects (`signal kill` during bootstrap). The app/unit test targets pass; unclear whether the remaining failure is an environment-only UI-test issue or a pre-existing app-launch problem in the macOS UI harness.
- Assumed legacy numbered roadmap files should keep working during the priority-bucket transition. `ingest` still reads numeric items as a fallback, while PM-linked waves now prefer bucketed `p0`–`p3` filenames.
- Assumed numeric local roadmap items synced to PM should default to shared bucket `P1` when no explicit bucket prefix exists. That preserves a sensible "clear next step" fallback without inventing fake exact ordering.
