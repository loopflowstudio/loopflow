# Questions

- Chosen headless (concept review): hold every interactive `lf` launch in `lf holder run` whenever the Machine server is running, with 15-minute idle-unviewed retirement for standalone chats and automatic direct-terminal fallback when no server is running. Opening any running `lf` session in Desktop or another Rex pane is always a lossless `View` (**Open here**), never a `SIGTERM`/`SIGKILL` restart. Tradeoff: Rex sees `lf` instead of the provider as the foreground program (same as Task sessions) and `Ctrl+]` detaches instead of `Ctrl+Z` suspending. Passed over: promote-on-second-open with a follower relay (still kills mid-turn and disguises a kill as **Open here**), holding standalone chats forever without cleanup (leaks idle providers on closed tabs), and hard-requiring the server (breaks bare CLI without a server). See `scratch/concept-review.html` for numbered review questions (`1a`–`3b`).

- The direct launcher uses the connection that welcomed it. An unanswered
  `session.start` can already have started the provider, so report its error
  instead of launching a second copy in the caller.

- A delegated Skill keeps its launcher inside the held PTY to run its ordinary
  completion and checkpoint. The original invocation carries whether it owns
  that checkpoint across the relay, which otherwise looks like a new caller.
