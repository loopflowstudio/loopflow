# Open choices (LOO-977)

- The plan types four Codex failures from `codexErrorInfo` (usage limit,
  model not found, unauthorized, server overloaded), beyond the brief's
  apostrophe match. Drop slice 1's other three rows, or all of slice 1, if
  only the wording fix is wanted.
- Settled 2026-11-18: the failures carried the type. Seven rollouts under
  `~/.codex/sessions/2026/11/18/` hold
  `"codex_error_info":"usage_limit_exceeded"` beside the exact message. The
  app-server's camelCase lines for the same failure are now the decode fixture.
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
- The stored Session provider now follows a headless retry that moves
  provider or model. `lf session list` and steer routing read that column, so
  a step that moved shows the agent it is running on. The same gap in the
  other direction (Claude to Codex leaves `provider='claude'`, so Codex's
  close check was skipped) is closed by the same write; no test drives that
  direction.
- `release.rs` now counts every `subscription_limit_text` phrase as spent
  quota, which adds "you've hit your …" (any Claude window),
  "subscription limit" and `rate_limit_reached` to what release notes report
  as quota.
- From qa: headless work outside a Graph (`lf -b : '…'`) with
  `agents: [codex, claude:haiku]` stops on Codex's spent limit and prints
  "Nothing is left in its order after codex; add another agent to `agents`".
  The list already has another agent; only Graph steps walk it. Before this
  change that stop read `agent exited with code 1: codex_error: …`. Reword the
  advice for work that never walks the order, or let it walk?
- From qa: a Codex that was never signed in fails with
  `codexErrorInfo: {"httpConnectionFailed":{"httpStatusCode":401}}` and
  "unexpected status 401 Unauthorized: Missing bearer or basic authentication
  in header", which is `codex_error` and stops the step with Claude next in
  the order. A revoked login moves on; a missing one does not.
- From qa: stopping a waiting `lf -b graph` with SIGTERM leaves its step
  (`lf --batch … skill <name>`) waiting out the usage limit as an orphan.
- From qa: a step that fails on Codex prints two `Error:` lines, the second
  the raw error object (`Error: say: {"additionalDetails":null,…}`). The
  installed 0.16.0 does the same.
