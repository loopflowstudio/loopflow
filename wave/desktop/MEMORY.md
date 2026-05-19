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
  terminal, remove the temporary exit file, then keep the tmux pane alive via
  `exec "${SHELL:-/bin/zsh}"` so the pane stays attachable. Startup reconcile
  re-arms palette exit-file watchers when tmux is still live and completes rows
  whose tmux session died while lfd was down.
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
- **lfd readiness probes are split by route prefix.** `/health` is an
  unauthenticated root route, while most API calls live under `/v0`. Validation
  scripts that wait for local lfd should probe `http://127.0.0.1:2486/health`
  instead of `/v0/status` (`/status` is root and auth-protected).

## Patterns (verified 2026-05-19, native-chat-ux M1 implementation)

- **Assistant markdown parsing now lives in LoopflowCore.** The canonical model is
  `MarkdownBlock` plus `parseMarkdownBlocks(_:)` /
  `parseStreamingMarkdownBlocks(_:)` in
  `swift/LoopflowCore/Models/MarkdownBlock.swift`. The old
  `MessageSegment` / `parseMessageSegments` view-layer implementation was
  deleted from `WaveSessionView.swift`; do not reintroduce a parallel parser in
  Concerto views.
- **Message rows use a split rich/final vs cheap/streaming path.**
  `MarkdownBlockCache` in `swift/Concerto/Views/MessageRow.swift` caches
  finalized assistant blocks by `(message.id, content.count)`. While
  `showStreamingCursor` is true it bypasses that cache and calls the cheap
  streaming parser, which preserves fence splitting but skips inline markdown
  parsing and syntax highlighting.
- **Assistant block rendering is centralized.**
  `swift/Concerto/Views/AssistantMarkdownBlocksView.swift` renders paragraph,
  heading, list, blockquote, code, diff, and rule blocks. `diff` / `patch`
  fenced blocks route to the existing `DiffLinesView`; normal code blocks route
  to `CodeBlockView`.
- **Syntax highlighting is heuristic and in LoopflowCore.** `SyntaxHighlighter`
  tokenizes the supported chat languages (swift, rust, python, shell, json,
  yaml, toml, markdown, diff/patch) into token kinds. Concerto maps those token
  kinds to palette colors in `CodeBlockView`; there is still no new Swift
  package or JS/tree-sitter dependency.
- **Selectable assistant text now accepts attributed content.** macOS
  `SelectableAssistantMessageTextView` and iOS `SelectableAssistantTextView`
  accept `AttributedString` so inline markdown can flow through the existing
  quote/emoji selection affordances for paragraph blocks.

## Patterns (verified 2026-05-19, native-chat-ux review-design)

- **Chat markdown parsing is in the view layer, not Core.** Current parser is
  `parseMessageSegments` + text/code-only `MessageSegment` enum at
  `swift/Concerto/Views/WaveSessionView.swift:691-753`, cached by
  `MessageSegmentCache` keyed on `content.count` (`cachedContentLength`) in
  `swift/Concerto/Views/MessageRow.swift`, tested in
  `swift/ConcertoTests/WaveSessionViewTests.swift`. native-chat M1 *relocates*
  this into `LoopflowCore` (`MarkdownBlock`/`parseMarkdownBlocks`) and deletes
  the old enum/parser/tests — not a parallel impl.
- **iOS already does inline markdown; macOS does not.** iOS:
  `NSAttributedString(markdown:options:.init(interpretedSyntax:.inlineOnlyPreservingWhitespace))`
  at `swift/Concerto/Platform/iOS/SelectableAssistantTextView.swift:112-114`.
  macOS renders raw text via `AutosizingSelectableTextView`, `isRichText =
  false`, `swift/Concerto/Platform/macOS/Views/SelectableAssistantMessageTextView.swift:85`
  (`:17` assigns the string verbatim). M1 unifies on the iOS technique.
- **`DiffLinesView` exists but is wired only to transcript tool cards.**
  `swift/Concerto/Views/DiffLinesView.swift:47-125` (parser `parseDiffLines()`
  `:22-43`), reached from `TranscriptItemCardView` at
  `WaveSessionView.swift:607`. Routing ` ```diff ` message blocks to it is
  *new wiring*, not a reroute.
- **Session resume/replay is `afterSeq: nil`, not `0`.**
  `SessionState.joinSession(id)` (`swift/LoopflowCore/State/SessionState.swift:218`)
  then `reconnectIfNeeded()` → `startStream(... afterSeq: nil ...)`
  (`:327-351`); the `replayCompletedLastSeq` envelope promotes replay→`.live`
  (`:452-462`). Ended sessions have no live tail — replay completes and stops.
  This is the exact path live reconnect already uses.
- **`Session` has `wave_run_id`, NOT `wave_id`.** Model at
  `rust/loopflow/src/lfd/sessions/types.rs:403-415` carries
  `{id, harness, status (SessionStatus), wave_run_id, provider_session_id,
  config, created_at, ended_at}`. `wave_id`/`wave_name` for any session DTO
  must derive via `JOIN wave_runs wr ON wr.id = s.wave_run_id` (then `waves`
  for the name) — the join the usage route already does. No `title` or
  `message_count` columns; derive from `session_events`.
- **Session-list query is per-wave only.** `list_sessions_for_wave`
  (`rust/loopflow/src/lfd/store/sqlite.rs:784`) and `list_events_for_sessions`
  (`:747`); usage route calls both at
  `rust/loopflow/src/lfd/http/routes/usage.rs:81,88`. No
  `list_sessions_for_repo`. There is no `GET /sessions` list route yet
  (`lfd/http/mod.rs` has create/get/input/events/usage only). native-chat M2's
  history is **per-wave for v1** (`wave_id` required); cross-wave is v2.
- **`session_events` is append-only, no prune.** PK `(session_id, seq)`; zero
  `DELETE`/prune paths in `sqlite.rs`/`postgres.rs`. Replay query
  `list_session_events(session_id, after_seq)` (`sqlite.rs:675`) is the same
  one the live-reconnect SSE handler uses (`lfd/http/routes/sessions.rs:124`).

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
- Headless loopflow runs set `LF_RUN_ID`; Rust tests that assert generated
  journal ids or branch-derived ingest claims must clear that environment
  explicitly or full `cargo test -p loopflow` fails only under agent-driven
  validation.
