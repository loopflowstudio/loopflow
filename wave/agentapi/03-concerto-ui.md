# 03: Concerto UI

Minimal chat panel for interactive sessions. Connect to the session API, render events, send input.

## What exists after this

When a session is active, Concerto shows a chat panel with the event transcript, a text input box, and an End button. Closing and reopening Concerto reconnects to the running session and replays history.

## What to build

- Chat transcript rendered from SSE event stream (`GET /sessions/{id}/events` — replay + follow)
- Text input composer → `POST /sessions/{id}/input`
- End button → `DELETE /sessions/{id}`
- Reconnect: check for active sessions, reattach to event stream
- Typed item rendering: the event model provides `Command`, `File`, `Message`, `Thought`, and generic `Tool` items. Render each inline with appropriate chrome (command name + output, file path + status, message text). No need for rich tool visualization — just enough to distinguish item types visually.

## What not to build

- Rich tool visualization (file diffs with syntax highlighting, terminal emulation)
- Multi-session views
- Advanced layout or theming beyond existing Concerto patterns
- Approval/permission UI (everything runs in bypass mode)

## What Phases 01–02 give us

The event model is richer than originally planned. Items are typed (`Command`, `File`, `Message`, `Thought`, `Tool`) with lifecycles (`ItemStarted → ItemUpdated → ItemCompleted`) and carry `turn_id` for grouping. This means Concerto can distinguish between a command executing and a file being edited without parsing text — just match on the item type.

Two working adapters (Codex and Claude) are available for testing. This means the UI can be validated against both providers early, catching any accidental Codex-specific assumptions.

**`DiffUpdated` is provider-dependent.** Codex emits `DiffUpdated` with a turn-level diff; Claude does not. The UI should render diffs when available but not rely on them for core functionality. The chat transcript and typed item rendering are the universal building blocks.

**Busy turn rejection is clean.** Sending input while a turn is in progress returns 409 without disrupting the session. Concerto should handle this gracefully — disable the input composer while a turn is active, or show a transient indicator if a 409 arrives.

## Done when

- Session event transcript visible in Concerto
- User can send input and see agent responses streaming
- End button stops session
- Close/reopen Concerto reconnects with full history replay
