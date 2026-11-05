# Open questions

- Codex `/new` and `/fork`: the Session follows the new thread the way Claude's
  `/clear` does, and the thread has lf's instructions and hooks because they are the
  private app-server's configuration (implemented 2026-11-05; sessionFlags verified on
  0.166.0). Option (b), an observer interjection, is not needed.
- lf's ~10 KB instructions now ride argv as `-c developer_instructions=...` on
  `codex app-server`. #1552 (Jack Heart, 2026-10-14) moved instructions off argv because
  large arguments slowed terminal wrappers; its target was mainly the first request
  (hundreds of KB), which still never rides argv. Codex 0.166.0 has no file form for
  developer instructions (`model_instructions_file` replaces the base prompt). Assumed
  acceptable: Linux caps one argument at 128 KiB, so instructions near that size would
  fail to spawn. `native_codex.py` now allows 64 KiB per server argument, 1 KiB per
  client argument.
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
