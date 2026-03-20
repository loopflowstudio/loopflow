## Try it!

```bash
# Core validation
cargo test --all
uv run pytest python/tests/
swift test --package-path swift

# API / websocket smoke
tests/e2e/test_smoke.sh
uv run pytest tests/e2e/test_api_smoke.py tests/e2e/test_concurrent_clients.py -v

# Run the app from this branch
uv run python scripts/concerto-dev.py run-debug
```

In Concerto:
- select a wave and confirm the old scroll-view detail panel is gone
- the default workspace should open with **Roadmap**, **Runs**, and **Terminal** panes
- press **Cmd+K** and switch waves or open/focus **README** / **Launcher** panes
- pick a waiting wave and confirm the interactive session still takes over instead of leaving you stranded in the workspace

## Intent

This change turns the wave detail area into a real workspace. Instead of one long vertical panel, each wave now gets a persistent multiplexer layout with panes for the concerns people actually switch between: roadmap, runs, readme/markdown, launcher, diff, and terminal. The branch also lands the backend session/state work needed to make that workspace reliable, and aligns the built-in flow/step taxonomy and docs with the new build/garden/wave/VSM model.

## Assumptions

- Concerto continues to treat Ghostty-backed terminal panes as a macOS-only capability.
- Waves without worktrees should still be explorable; only terminal-style panes are allowed to degrade to placeholders.
- Interactive waiting state remains higher priority than ordinary workspace browsing.
- Existing wave content (`README`, roadmap markdown, runs) stays the source of truth for the new panes.

## Key decisions

- Persist layout and focus per repo + wave via `MultiplexerStore`.
- Start new workspaces with an opinionated layout rather than an empty canvas.
- Make Cmd+K focus existing panes before creating duplicates.
- Promote terminal sessions into explicit `lfd` state so embedded terminals and waiting-wave routing have a shared source of truth.
- Rename/reorganize built-in flows and steps around `build`, `garden`, `wave/*`, and `vsm/*` so docs, YAML, and UI all speak the same language.

## Not included

- No cron/workers pane.
- No direct PM-backed roadmap pane.
- No full drag/drop pane editor.
- No launcher/launchpad merge.
- No sidebar-collapse redesign.

## Validation

Passed locally on March 20, 2026:

- `cargo fmt --check`
- `cargo clippy -- -D warnings`
- `cargo test --all`
- `uv run pytest python/tests/`
- `swift test --package-path swift`
- `tests/e2e/test_smoke.sh`
- `uv run pytest tests/e2e/test_api_smoke.py tests/e2e/test_concurrent_clients.py -v`

Attempted locally on March 20, 2026:

- `cd swift && xcodegen generate && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO`
  - the app/unit suites ran, but `ConcertoUITests-Runner` exited early before finishing bootstrap on this machine
