# Embedded terminal build driver — review

## What was implemented

Added an lfd-owned create/attach path for embedded build terminals. `POST /v0/terminal-sessions` now launches a palette terminal session for `{wave_id, flow, worktree, agent}`, returns both the persisted `TerminalSession` and tmux attach info, and records the launch as `source: "palette"` with the selected `agent`.

Palette sessions run `lf <flow> -m <agent>` inside daemon-owned tmux, write the flow exit code to `.lf/tmp/terminal-sessions/<id>.exit`, mark the lfd row terminal, remove the temporary exit file, then keep the pane alive via `exec $SHELL` so the embedded pane remains attachable after the flow exits. Startup reconcile reattaches palette completion watchers for live tmux sessions and completes stale rows whose tmux session died while lfd was down.

Concerto's multiplexer terminal panes now bind to lfd terminal session ids (`terminalSessionId`) and attach through the existing `RepoState.attachTerminalSession` contract. Palette/launchpad actions create lfd sessions and store the returned session id in the pane layout; the pane header shows `session.agent`.

## Key choices

- Reused `TerminalSession.source` and `TerminalSession.agent` instead of adding new `interactive` or `provider` fields. This avoids a database migration and keeps Rust/Swift/Python DTO mirrors aligned.
- Used `source == "palette"` as the lifecycle discriminator. Wave-run tmux sessions still exit and complete by tmux death; palette sessions complete by exit-file and stay alive as shells.
- Renamed `PaneConfig.terminalSessionName` to `terminalSessionId` rather than preserving a compatibility shim. Old persisted layouts may lose terminal bindings once; new layouts are honest about the durable identity they store.
- Kept client-side `TmuxSession` only for the workspace shell escape path. Multiplexer build terminals are daemon-owned.
- Added DTO fixtures for both the new create request and the existing terminal-session response shape.
- Gitignored `.lf/tmp/` and removed two stray `.exit` files that a prior run committed. Palette sessions use `.lf/tmp/terminal-sessions/<id>.exit` as a temporary completion signal and remove it after the row is completed, so that runtime directory must not be tracked.

## How it fits together

The command palette and launchpad call `RepoState.createTerminalSession`, which posts to lfd. lfd creates a persisted `TerminalSession`, launches tmux, returns attach info, and watches the palette exit file. Swift stores the returned session id in the focused or replacement terminal pane; `TerminalPaneView` loads/attaches that session through the same attach RPC already used by `TerminalWorkspaceView`.

## Risks and bottlenecks

- Palette sessions intentionally keep tmux alive after flow completion, so users can accumulate shells. Existing cancel/close kills the lfd session; exit files are cleaned after completion, but dogfooding should reveal whether an idle tmux sweep is needed.
- The verify script uses the live local lfd port and terminates any existing process on `:2486`; that is appropriate for validation but not a background-safe smoke test.
- Remote tmux attach remains an external-terminal escape hatch; the embedded pane currently shows a remote-tmux unavailable state.
- The launch command uses the configured wave direction/area and selected agent. If a future caller needs stay-alive lifecycle outside the palette, `source == "palette"` will need to become a more general provenance/lifecycle model.

## What's not included

- Native chat rendering/history/composer work.
- A user-shell create endpoint for empty terminal panes.
- tmux pane/window management inside a terminal session.
- A schema migration or new `TerminalSession` fields.
- Full UI automation for the Concerto walkthrough.

## Validation

- `cargo fmt --check` — passed
- `cargo clippy -- -D warnings` — passed
- `cargo test -p loopflow` — passed (958 passed, 2 ignored in lib plus integration/doc tests)
- `cargo test -p loopflow --test dto_fixtures` — passed
- `uv run pytest python/tests/test_dto_fixtures.py -q` — passed (4 passed)
- `uv run pytest python/tests/` — passed (139 passed)
- `swift test --package-path swift` — passed (335 Swift Testing tests plus XCTest package tests)
- `swift test --package-path swift --filter DTOFixtureTests` — passed (4 selected fixture tests)
- `uv run python scripts/verify_embedded_build_driver.py --skip-build` — passed; launched a palette terminal, observed terminal row completion, and verified tmux stayed attachable

Gate also fixed two test assumptions exposed by the loopflow-run environment: Rust tests that expect generated run ids or branch-based ingest claims now explicitly clear `LF_RUN_ID` for those assertions.
