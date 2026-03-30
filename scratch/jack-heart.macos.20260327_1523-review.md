# Roadmap pane redesign — validation results

## Passed

- `swift test --package-path swift`
- `cd swift && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO -only-testing:ConcertoTests`

## Known failure

- `xcodebuild test -scheme Concerto` without `-only-testing` fails because `ConcertoUITests-Runner` exits early before the screenshot pipeline test can establish a connection. Pre-existing issue, not caused by this branch.

## Hover check

Reviewers should sanity-check the hover play button affordance on a live build (`uv run python scripts/concerto-dev.py run-debug`).
