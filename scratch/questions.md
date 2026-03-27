# Open questions

- The new workspace-level "Open Terminal" and "Open Internally" actions are implemented only for local worktrees. Remote targets still use the existing IDE/SSH flows because the new Ghostty+tmux path assumes a local Ghostty install and local tmux session.

- `xcodebuild test -scheme Concerto` still ends with a `ConcertoUITests-Runner` bootstrap crash in this local environment even though the package tests and in-process Concerto tests finish green. The failure happens before `ScreenshotPipelineTests` establishes a UI test connection, so it looks environmental rather than tied to the Ghostty/session changes.
