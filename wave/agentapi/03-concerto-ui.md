# 03: Concerto UI

Minimal chat panel for interactive sessions. Connect to the session API, render events, send input.

## What exists after this

When a session is active, Concerto shows a chat panel with the event transcript, a text input box, and an End button. Closing and reopening Concerto reconnects to the running session and replays history.

## What to build

- Chat transcript rendered from SSE event stream (`GET /sessions/{id}/events` — replay + follow)
- Text input composer → `POST /sessions/{id}/input`
- End button → `DELETE /sessions/{id}`
- Reconnect: check for active sessions, reattach to event stream
- Tool events rendered inline (tool name + output, when available)

## What not to build

- Tool visualization (file diffs, terminal output panels)
- Multi-session views
- Advanced layout or theming beyond existing Concerto patterns
- Approval/permission UI (everything runs in bypass mode)

## Done when

- Session event transcript visible in Concerto
- User can send input and see agent responses streaming
- End button stops session
- Close/reopen Concerto reconnects with full history replay
