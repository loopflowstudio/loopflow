# Open questions

- Codex `/new` and `/fork` now work inside a Session (the relay refused them). The
  observer records only the Session's thread, so a conversation started that way
  goes unrecorded and `lf resume` returns to the original. Assumed acceptable for
  this Task; following a new thread is a separate decision for Jack Heart.
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
