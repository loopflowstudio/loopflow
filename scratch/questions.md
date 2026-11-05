# Open questions

- Codex `/new` and `/fork`: the Session follows the new thread the way Claude's
  `/clear` does (implemented 2026-11-05). Concept review 2 found that the missing lf
  context on a thread the person starts comes from lf configuring per thread, not from
  a Codex limit. The fix is to move the configuration onto the private app-server (see
  the plan). Option (b), an observer interjection, is no longer needed. If Codex turns
  out to ignore `sessionFlags` for client-started threads, the question returns.
- Killing the terminal owner still ends the TUI: it holds the PTY, for Claude too.
  The brief's "kill the lf owner" demo is met for the launcher and observer, which
  are the processes that hosted the relay.
- An attachment-moved TUI can write for up to ~1s until the observer ends it, and
  indefinitely if the observer is dead. The relay's per-write fence cannot exist
  without a relay.
- lf now starts the thread, so Codex's `codex_tui` dynamic tools (`list_threads` and
  friends, declared by the TUI in its own `thread/start`) are absent from a Session.
  They manage other tasks on the same app-server, and a Session's server is private
  to it. Assumed acceptable; the alternative is a client-started thread with lf's
  configuration supplied another way, which Codex 0.166.0 does not offer per thread.
- Two real-Codex fixtures were already broken on main and stay broken; neither is
  caused by this Task and neither runs in CI. `codex_connect.py --planning-peers`
  writes the renamed `task_workflows` table; past that it reaches the same `lf resume`
  of a Run that is refused (read from the code, not run). `actual_agent_process_children...` (ignored) passes its Codex half on
  0.166.0, then asserts on Process rows for `lf session list`, a read that records
  none since `changes_work`. Left unrepaired; each needs a re-author or retirement.
