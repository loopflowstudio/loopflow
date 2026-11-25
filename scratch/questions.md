# Questions

- Chosen headless (pre-implementation concept review, built on `HEAD`): hold
  every interactive `lf` launch in `lf holder run` whenever the Machine server
  is running, with 15-minute idle-unviewed retirement for standalone chats and
  automatic direct-terminal fallback when no server is running. Opening any
  running `lf` session in Desktop or another Rex pane is always a lossless
  `View` (**Open here**), never a `SIGTERM`/`SIGKILL` restart. Tradeoff: Rex
  sees `lf` instead of the provider as the foreground program (same as Task
  sessions) and `Ctrl+]` detaches instead of `Ctrl+Z` suspending. Passed over:
  promote-on-second-open with a follower relay, holding standalone chats
  forever without cleanup, and hard-requiring the server.

- Chosen headless (post-build concept review, built in Pass 2): record
  intentional retirement in `agent_sessions.agent_end_reason` (additive
  migration, cleared in `attach_in`) and record `agent_exit_code` on waited
  native launches (`spawn_native` in `native_session.rs`). Retired chats,
  retired Skills, and retired completed/abandoned Task conversations report
  `closed HH:MM (<reason>)` (`WaitingForYou`, capture `"completed"`, no
  `"<provider> exited with 137."` on the saved screen) and never auto-restart
  as crashes; cleanly quit Skills and fallback chats report `quit HH:MM`
  instead of `died (no exit recorded)`. Passed over: leaving retired chats
  showing `died (exit 137)` (~64% of everyday chats), storing `close_reason`
  only on `held_terminals` (leaks across unheld fallback resumes), and
  renaming `WorkSession`/`Chat` across the whole product. See
  `scratch/concept-review.html` for numbered review questions (`1a`–`4b`).

- The direct launcher uses the connection that welcomed it. An unanswered
  `session.start` can already have started the provider, so report its error
  instead of launching a second copy in the caller.

- A delegated Skill keeps its launcher inside the held PTY to run its ordinary
  completion and checkpoint. The original invocation carries whether it owns
  that checkpoint across the relay, which otherwise looks like a new caller.

- A delegated Skill already prepares a NativeExit before binding its Session.
  Keep that receipt for retirement, when the launcher cannot finish its own
  capture. Record the ending against its agent generation. A terminal-only
  UPDATE can hit a resumed fallback chat because held_terminal_id survives
  reattachment.

- Waited launches keep the provider's shell status. The foreground interrupt
  flag also covers SIGHUP and SIGTERM. Rewriting all those exits to 130
  would report an external kill as a quit.
