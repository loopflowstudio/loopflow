# Open choices (LOO-977)

- The plan types four Codex failures from `codexErrorInfo` (usage limit,
  model not found, unauthorized, server overloaded), beyond the brief's
  apostrophe match. Drop slice 1's other three rows, or all of slice 1, if
  only the wording fix is wanted.
- Settled 2026-11-18: the failures carried the type. Seven rollouts under
  `~/.codex/sessions/2026/11/18/` hold
  `"codex_error_info":"usage_limit_exceeded"` beside the exact message. Still
  uncaptured: the app-server's camelCase JSON for the same failure; slice 1's
  demo stands in for it.
- `modelNotFound` now moves a step to the next agent where it stopped the
  Graph before (98 failures on 2026-11-13, `gpt-6.1-astra` on a ChatGPT
  account). A misspelled model in config would move on quietly instead of
  stopping; the move is recorded in `agent_limit_recoveries`.
- Later: `release.rs` classifies a failed agent by re-reading its words
  because `CommandError` carries no `AgentFailure`.
- Later: `ConversationEvent::Error.code` is a string holding two ideas, who
  reported (`codex_error`, `claude_exit`) and what happened
  (`provider_rate_limited`). A typed field on the wire event is a DTO change
  across Rust and Swift.
