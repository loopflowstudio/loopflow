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
