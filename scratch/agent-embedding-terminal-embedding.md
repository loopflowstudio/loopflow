# 02: Terminal Embedding

## Try it

```bash
uv run python scripts/concerto-dev.py run-debug
```

Start two local waves that pause on interactive steps. In one repo window, verify:

- each waiting wave opens a separate Ghostty-backed terminal tab bound to that wave
- the selected tab shows wave context, active attention, recent PR/commit history, and quick actions in a sidebar
- switching tabs does not destroy the other terminal surface
- exiting one terminal with status 0 resumes that wave in `lfd`; exiting non-zero marks the run failed
- no coding-session view in the repo window reformats agent output into chat bubbles while the terminal session is active

## Measure

Use `terminal_sessions` telemetry in `lfd` to measure adoption and friction.

- Baseline before this work: **0%** of interactive coding sessions complete inside Concerto; all require chat UI or an external terminal.
- Capture after launch:
  - `in_app_rate = completed_terminal_sessions_started_from_concerto / all_interactive_wave_steps`
  - `resume_latency = terminal_session_completed_at -> wave_resumed_at`
- Better looks like:
  - `in_app_rate > 70%` for local interactive wave steps
  - p95 `resume_latency < 2s`

Verification query:

```bash
sqlite3 ~/.lf/lfd.db '
  select source, status, count(*)
  from terminal_sessions
  group by source, status
';
```
