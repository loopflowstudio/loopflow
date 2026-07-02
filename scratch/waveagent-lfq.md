---
requires: existing code
produces: code changes
---
Turn lfq into a session cockpit: `lfq sessions` lists live agent/dispatch sessions,
`lfq attach <id>` drops you into one over tmux.

Design context: `scratch/waveagent-sessions.md` (read "lfq: two verbs, not a cockpit"
and "Interactive steps: this is Attention's fit moment"). Scope is exactly two verbs —
do not reshape existing lfq commands, do not touch Rust.

## Goal

lfq today is wave-centric (`list/show/run/stop/logs/...`) and knows nothing about
terminal sessions. The lfd HTTP API already exposes them. Add two commands that let a
human see and enter the live sessions a wave and its dispatched subagents run in —
especially to answer an interactive step a subagent is parked on.

## Endpoints (already exist on lfd)

- `GET /v0/terminal-sessions?wave_id=<id>&statuses=<csv>` — list sessions. DTO is
  `TerminalSessionDto` (`rust/loopflow/src/lfd/http/dto.rs`, ~line 242): fields
  include `id`, `wave_id`, `wave_run_id`, `step`, `source`, `status`, `tmux_name`.
- `POST /v0/terminal-sessions/{id}/attach` — marks attached and returns tmux
  connection info (includes the `tmux_name` / status `"attached"`). Rejects
  non-tmux/terminal sessions with an error.
- `GET /v0/attention?...` — list attention items (`AttentionItemDto`), each carrying a
  `terminal_session_id` and a `kind` (`interactive`/`algedonic`) + `status`
  (`surfaced`/`viewed`/`resolved`). Use to flag which sessions are parked waiting on a
  human.

## Workflow

1. **Client methods** in `python/loopflow/client.py`:
   - `list_terminal_sessions(self, wave_id: Optional[str] = None, statuses: Optional[list[str]] = None) -> list[dict]`
   - `attach_terminal_session(self, session_id: str) -> dict`
   - `list_attention(self, status: Optional[str] = None) -> list[dict]` (if not already present)
   Follow the existing `_request_json` pattern in that file. Return parsed payloads
   (dicts are fine here — these are read models for CLI display, not the typed `Wave`
   model). Resolve a wave *name* to id via the existing wave lookup when the user
   passes a name.

2. **`lfq sessions [wave]`** in `python/loopflow/cli.py`:
   - Optional positional `wave` (name); when given, filter by that wave's id.
   - Render a scannable table: wave, session id (short), role (derive: `wave_run_id`
     empty + `source=wave_agent` → "agent"; empty + `source=palette` → "palette";
     set → "dispatch"), step, status, and a **needs-input** flag when an unresolved
     `AttentionItem(Interactive)` references that session id.
   - Follow the table style of the existing `list`/`show` commands in this file.

3. **`lfq attach <session_id>`** in `python/loopflow/cli.py`:
   - POST attach, read `tmux_name` from the response.
   - If attachable, replace the process into the tmux client:
     `os.execvp("tmux", ["tmux", "attach", "-t", tmux_name])`. If tmux isn't available
     or the session isn't attachable, print a clear error and exit non-zero.
   - Accept a short id prefix if that's how other commands resolve ids; otherwise exact
     id is fine.

4. **Tests** (`python/tests/`): mock the HTTP layer (network is a side effect), assert
   on behavior, not mock-call plumbing (see CLAUDE.md):
   - `lfq sessions` renders a row per session from a fake payload, and shows the
     needs-input flag when a matching unresolved interactive attention item exists.
   - `lfq attach` resolves the tmux name and execs tmux with it (patch `os.execvp` /
     the tmux exec point and assert the target name); a non-attachable session errors
     cleanly.

## Guardrails

- Typer conventions (CLAUDE.md): lowercase short flags, sensible defaults, pass through.
- Don't reshape existing lfq commands. Two new verbs only.
- Imports at top of file. Type hints on public functions.
- `uv run pytest python/tests/` must pass. Keep tests short and behavior-focused;
  delete anything flaky rather than adding retries.

## Output

`lfq sessions` and `lfq attach <id>` working against a running lfd, with tests.
