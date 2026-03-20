# Branch review — jack-heart.agent-embedding.20260320_1445

## What was implemented

This branch replaces Concerto's scroll-style wave detail panel with a pane-based workspace and lands the backend/runtime work needed to make that workspace useful.

The user-visible center of the change is the new multiplexer view: selecting a wave now opens a persistent layout with roadmap, runs, README/launcher, diff/markdown, and terminal-style panes instead of one long detail stack. Cmd+K can switch waves and focus or create panes by type. Interactive sessions still take over when a wave is blocked on input.

Under that UI, the branch also adds terminal-session plumbing in `lfd` and the Swift client, expands wave/run DTOs and event handling, and reshapes built-in flow/step taxonomy around `build`, `garden`, `wave/*`, and `vsm/*` concepts. README/docs/wave docs were updated to match the new model.

## Key choices

- **Per-wave persisted layouts** — `MultiplexerStore` keeps layout and focus state keyed by repo + wave so each wave can have its own workspace.
- **Default workspace is opinionated** — new waves open with roadmap + runs on the left and a terminal on the right rather than an empty canvas.
- **Focus, don't duplicate** — palette actions focus an existing pane of the requested type when possible and only split/create when needed.
- **Interactive sessions still win** — waiting waves keep their takeover behavior so the new workspace does not hide unblock-now work.
- **Terminal sessions are first-class backend state** — the Rust side now tracks terminal sessions explicitly instead of treating embedded terminal behavior as purely local UI state.
- **Flow naming was normalized to the new mental model** — `build`, `garden`, `wave`, and `vsm` names now align docs, builtins, and app surfaces.

## How it fits together

`lfd` now emits richer wave/session state, stores terminal sessions, and exposes the extra HTTP routes/DTO fields the app needs. In Swift, `RepoState` consumes that state, routes waiting waves into interactive sessions or terminal surfaces, and hands normal wave viewing to `MultiplexerStore` + `MultiplexerView`. `CommandPalette`, keyboard routing, and the new pane views sit on top of that shared layout/store layer.

## Risks and bottlenecks

- **Concerto UI test runner is still the weakest validation point locally.** `swift test --package-path swift` passes, but full `xcodebuild test` on this machine ended with an early `ConcertoUITests-Runner` exit after the app/unit suites completed.
- **Terminal/session behavior spans Rust + HTTP + Swift.** Regressions here are more likely to show up as state-sync issues than compile errors.
- **Persisted pane layouts can go stale across future layout schema changes** if migrations are not kept in sync.
- **Ghostty-backed panes remain a platform-specific dependency** and are the most environment-sensitive part of the workspace.

## What's not included

- No cron/workers pane yet.
- No direct PM-provider roadmap pane; roadmap still comes from existing wave content.
- No sidebar-collapse redesign.
- No full pane editor beyond split/focus/close flows.
- No attempt to merge `launcher` and `launchpad` yet.

## Validation

Passed on March 20, 2026:

- `cargo fmt --check`
- `cargo clippy -- -D warnings`
- `cargo test --all`
- `uv run pytest python/tests/`
- `swift test --package-path swift`
- `tests/e2e/test_smoke.sh`
- `uv run pytest tests/e2e/test_api_smoke.py tests/e2e/test_concurrent_clients.py -v`

Attempted on March 20, 2026:

- `cd swift && xcodegen generate && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO`
  - First run hit a transient linker write failure in `ConcertoUITests`.
  - Second run built and executed app/unit suites, then failed when `ConcertoUITests-Runner` exited early before establishing its test connection.
