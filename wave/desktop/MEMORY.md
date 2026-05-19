# Desktop wave memory

## Patterns (verified 2026-05-18, embedded-terminal kickoff review)

- **lfd `TerminalSession` already models provenance + agent.** Struct at
  `rust/.../terminal_session.rs:79-106`, Swift mirror
  `swift/LoopflowCore/Models/TerminalSession.swift:21-70`. Has `source`
  ("wave_step"/"user_shell") and `agent` ("claude"/"interactive"/…). Do **not**
  invent parallel `interactive`/`provider` fields — reuse these.
- **TerminalSession persistence is Postgres only** (`store/postgres.rs:893-925`,
  `INSERT INTO terminal_sessions`, explicit 16-col list). Any new field = schema
  migration. `source`/`agent` are `String` columns, so new *values* are free.
- **Attach contract ships and is proven**, not just wired: `POST
  /v0/terminal-sessions/{id}/attach` → `TerminalConnectionInfoDto
  {session_name,host,cwd,status}` (`http/routes/terminal_sessions.rs:89,290`).
  `TerminalWorkspaceView.swift:187-191` already builds `tmux attach-session`
  argv from it via `RepoState.attachTerminalSession` (`RepoState.swift:867`).
  The multiplexer pane is the *only* place still on a client-side `TmuxSession`.
- **Sessions only created in the wave executor** (`wave/mod.rs:320-346`); no
  `POST /terminal-sessions` create route exists (routes at
  `http/mod.rs:82-104`). Argv helper: `build_lf_step_command`
  (`helpers.rs:358`) — takes no model/cwd arg.
- **Flow lifecycle:** wrapped cmd ends `exit "$EXIT_CODE"`
  (`wave/mod.rs:689-693`); completion detected by `tmux has-session` polling in
  `wait_for_tmux_session_exit` (`:728-743`). Stay-alive requires `exec $SHELL`
  + exit-file watch instead.
- **DTO fixtures:** `tests/fixtures/dto/` (only `session.json`,
  `session_unsupported_input.json` today). Rust test
  `rust/loopflow/tests/dto_fixtures.rs`; Swift
  `swift/ConcertoTests/DTOFixtureTests.swift`. `TerminalSessionDto` at
  `http/dto.rs:245-268`.

## Learnings

- Kickoff line numbers drift fast — re-verify before citing in a design. The
  embedded-terminal kickoff was directionally right but off by 10-50 lines and
  pointed at a generic `store/mod.rs` when persistence is `store/postgres.rs`.
- The high-value review-design move here was catching invented fields that
  duplicate existing ones, not re-litigating the approach. The approach held.
