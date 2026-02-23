# 03: Concerto UI

Minimal chat panel for interactive sessions. Connect to the session API, render events, send input.

## What exists after this

When a session is active, Concerto shows a chat panel with the event transcript, a text input box, and an End button. Closing and reopening Concerto reconnects to the running session and replays history.

## What to build

- Chat transcript rendered from SSE event stream (`GET /sessions/{id}/events` — replay + follow)
- Text input composer → `POST /sessions/{id}/input`
- End button → `DELETE /sessions/{id}`
- Reconnect: check for active sessions, reattach to event stream
- Typed item rendering: the event model provides `Command`, `FileChange`, `McpToolCall`, `AgentMessage`, `Plan`, and generic `Tool` items. Render each inline with appropriate chrome (command name + output, file path + status, message text). No need for rich tool visualization — just enough to distinguish item types visually.

## What not to build

- Rich tool visualization (file diffs with syntax highlighting, terminal emulation)
- Multi-session views
- Advanced layout or theming beyond existing Concerto patterns
- Approval/permission UI (everything runs in bypass mode)

## What Phase 01 gives us

The event model is richer than originally planned. Items are typed (`Command`, `FileChange`, `McpToolCall`, etc.) with lifecycles (`ItemStarted → ItemUpdated → ItemCompleted`) and carry `turn_id` for grouping. This means Concerto can distinguish between a command executing and a file being edited without parsing text — just match on the item type.

## Done when

- Session event transcript visible in Concerto
- User can send input and see agent responses streaming
- End button stops session
- Close/reopen Concerto reconnects with full history replay
