# Desktop wave memory

## Patterns (verified 2026-05-19, embedded-terminal implementation)

- **lfd terminal provenance is `TerminalSession.source`; provider display is
  `TerminalSession.agent`.** Rust struct at
  `rust/loopflow/src/lfd/types/terminal_session.rs`, Swift mirror at
  `swift/LoopflowCore/Models/TerminalSession.swift`. Reuse these fields for
  embedded terminal lifecycle/provider UI; do not add `interactive` or
  `provider` synonyms.
- **Tmux-backed lfd sessions use source constants.** Existing wave-run tmux
  source is `"wave_step_tmux"` (`TMUX_TERMINAL_SOURCE`); palette launches use
  `"palette"` (`PALETTE_TERMINAL_SOURCE`). `TerminalSession::is_tmux_backed()`
  treats both as attachable.
- **TerminalSession persistence has SQLite and Postgres mirrors.** Rows map in
  `lfd/store/sqlite.rs` and `lfd/store/postgres.rs`; both use explicit column
  lists. New columns still require schema/mirror work, but new `source` values
  do not.
- **Attach contract remains the shared path.** `POST
  /v0/terminal-sessions/{id}/attach` returns `TerminalConnectionInfoDto
  {session_name,host,cwd,status}`. Swift terminal panes should call
  `RepoState.attachTerminalSession(_:)` and attach Ghostty to the returned tmux
  session; do not recreate a parallel client-side tmux name.
- **Palette create path now exists.** `POST /v0/terminal-sessions` takes
  `{wave_id, flow, worktree, agent}` and returns `{session, connection}`. The
  executor builds `lf <flow> --no-direction ... -w <wave> -m <agent>`, stores
  `source="palette"`, and launches tmux via lfd.
- **Palette lifecycle completion is exit-file based.** Wave-run tmux sessions
  still exit the shell and complete after `tmux has-session` goes false.
  Palette sessions write `.lf/tmp/terminal-sessions/<id>.exit`, mark the row
  terminal, then `exec "${SHELL:-/bin/zsh}"` so the pane stays attachable.
  Startup reconcile re-arms palette exit-file watchers when tmux is still live
  and completes rows whose tmux session died while lfd was down.
- **Multiplexer pane binding stores lfd session ids.** `PaneConfig` uses
  `terminalSessionId`; default terminal panes no longer synthesize
  `lf-<waveId>-<paneId>`. A pane without a session id shows an empty-state until
  a palette launch binds it.
- **Terminal pane config is now only durable identity, not launch intent.**
  `PaneConfig.launchCommand` and multiplexer config normalization were removed
  on 2026-05-19; palette launches create lfd terminal sessions directly and
  then bind `terminalSessionId`.
- **Palette sessions may be terminal in lfd while still attachable in tmux.**
  A succeeded palette row means the flow exited and the pane dropped into a
  shell, not necessarily that the tmux session is gone. Swift terminal panes
  should still attach by session id instead of hiding terminal-status sessions.
- **DTO fixtures now cover terminal sessions.** Fixtures in
  `tests/fixtures/dto/terminal_session.json` and
  `create_terminal_session_request.json` are asserted in Rust, Swift, and
  Python fixture tests.

## Preferences

- For this wave, prefer lfd-owned terminal sessions over Swift-owned tmux.
  Client-side tmux remains only for the older workspace user-shell escape path
  until a user-shell create endpoint exists.

## Learnings

- Kickoff line numbers drift fast — re-verify before citing in a design. The
  embedded-terminal kickoff was directionally right but off by 10-50 lines and
  pointed at a generic `store/mod.rs` when persistence is split across concrete
  store backends.
- The high-value review-design move here was catching invented fields that
  duplicate existing ones, not re-litigating the approach. The approach held.
- `cargo test -p loopflow dto_fixtures` filters by test name, not integration
  test file; use `cargo test -p loopflow --test dto_fixtures` to run that file.
