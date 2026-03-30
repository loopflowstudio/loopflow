# Open questions

- The new workspace-level "Open Terminal" and "Open Internally" actions are implemented only for local worktrees. Remote targets still use the existing IDE/SSH flows because the new Ghostty+tmux path assumes a local Ghostty install and local tmux session.

- `xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO` still ends with `ConcertoUITests-Runner` exiting early before it establishes a UI-test connection in this local environment, even though the package tests and in-process Concerto suites finish green.
