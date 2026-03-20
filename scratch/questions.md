# Questions

## Open validation blocker

- `xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO` failed twice on March 20, 2026 because `ConcertoUITests-Runner` was killed before it finished bootstrapping (`Early unexpected exit, operation never finished bootstrapping`). Pre-existing issue, not caused by this branch. Swift package tests and all non-UI validation passed.
