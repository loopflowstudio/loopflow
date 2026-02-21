# 03: Concerto UI

Minimal chat panel for interactive agents. Replace PTY/Ghostty terminal path with HTTP-backed conversation view.

## What exists after this

When a wave is waiting on an interactive agent, Concerto shows a chat panel with the event transcript, a free-text input box, optional choice buttons for structured prompts, and an End button. Closing and reopening Concerto reconnects to the running agent and replays history.

## What to build

- Chat transcript rendered from SSE event stream (replay + follow)
- Free-text input composer → `POST /agents/{id}/input`
- Choice buttons when `input_requested` has `single_choice` kind
- End button → `POST /agents/{id}/end`
- Reconnect: `GET /waves/{wave_id}/agents/active` → reattach to event stream

## What not to build

- Tool visualization (file diffs, terminal output panels)
- Multi-agent views
- Advanced layout or theming beyond existing Concerto patterns

## Done when

- Interactive agent transcript visible in Concerto
- User can send input and see agent responses
- End button stops agent and advances wave
- Close/reopen Concerto reconnects with full history
