# Review: swiftchatui streaming performance + Ghostty shell access

## What was implemented

- Moved transcript grouping, latest-assistant tracking, timestamp labeling, and transcript ID indexing into `SessionState`, so streaming deltas update cached state instead of forcing `WaveSessionView` to recompute O(n) derived data on every render.
- Memoized assistant message segment parsing inside `MessageRow`, switched transcript auto-scroll to the reduced-motion-aware animation helper, and pinned the streaming cursor identity so token streaming does less work and avoids cursor flicker.
- Added Ghostty as a first-class terminal target, wired local workspace **Open Terminal** / **Open Internally** actions to a shared tmux-backed shell, and aligned the rest of Concerto's default terminal entry points on the same default.
- Added focused regression coverage for the new terminal default and the new derived transcript state.

## Key choices

- **Derived transcript state lives in `SessionState`.** The mutation site now owns grouping and timestamp bookkeeping, which keeps the streaming hot path O(1) for in-place assistant deltas.
- **One default terminal constant.** `TerminalApp.defaultExternal` replaces scattered hard-coded `.warp` / `.ghostty` picks, so the command palette, quick actions, and workspace shell button cannot drift.
- **Ghostty launch uses explicit first-surface arguments.** `launchGhostty` now passes `--window-inherit-working-directory=false` and `--initial-command=shell:...` so a reused Ghostty process still lands in the requested repo and only the opened surface gets the attach command.
- **Message segment cache invalidates by content length.** That keeps the current append-only streaming case cheap without adding hashing or another view model layer.

## How it fits together

`SessionState` now owns both the transcript source array and the derived structures that the session UI renders. `WaveSessionView` and `MessageRow` became thinner consumers of that state, while the workspace terminal sidebar now creates or reattaches a tmux session and sends the attach command either to embedded Ghostty or to an external Ghostty window.

## Risks and bottlenecks

- The message segment cache assumes streaming appends text; if assistant content starts being edited in-place without a length change, the cache key would need to become stronger.
- The new workspace shell actions are intentionally local-only; remote targets still go through the existing SSH / IDE flows.
- Ghostty launching assumes the app is installed at `/Applications/Ghostty.app` and that the CLI flags supported by current Ghostty releases are available.
- I could not capture Instruments numbers in this headless gate pass, so the performance claim is backed by structural changes and tests rather than before/after traces here.
- `xcodebuild test -scheme Concerto` still ends with a `ConcertoUITests-Runner` bootstrap crash in this local environment before `ScreenshotPipelineTests` connects, so the UI-test leg is still blocked on environment stability.

## What's not included

- No markdown rendering overhaul, conversation history, or composer feature work beyond the streaming cache.
- No remote-shell Ghostty path or reattach UI beyond the local workspace buttons added here.
- No new terminal preference UI; this change standardizes the existing default and launcher behavior instead.

## Validation

- ✅ `swift test --package-path swift`
- ✅ `cd swift && xcodegen generate && xcodebuild build -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO`
- ✅ `cd swift && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' -skip-testing:ConcertoUITests CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO`
- ⚠️ `cd swift && xcodegen generate && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO`
  - Package tests, `ConcertoTests`, and the full in-process test suites completed green.
  - `ConcertoUITests-Runner` crashed before bootstrapping the screenshot UI test connection in this environment.
