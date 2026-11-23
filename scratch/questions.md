# Questions (LOO-1002, choices made for review)

- Antigravity is in scope (Task comment, 2026-11-23: "Scope includes Antigravity"). Its wake uses
  `agy`'s undocumented language-server RPC with a launch-set `ANTIGRAVITY_SERVER_CSRF_TOKEN`.
  Passed over: `agy acp`, which would replace the TUI instead of waking it; `--remote-control`,
  which relays through Google and needs a login; the `Stop` hook's `continue`, which works only
  as a turn ends and never while idle.
- Antigravity's wake file is recorded as `agent_inbox` by its launch, as Codex's endpoint is, not
  reported by its hook through an env var. Passed over: the hook route, which sends back to the
  store an address the launch already made, and records nothing when the hook can't reach the server.
- Antigravity's wake file and log live in `<LF_HOME>/context-deliveries/` beside the delivery, not
  `.lf/prompts/`. Passed over: the checkout's prompts dir, which moves with the checkout.
- Sync with main: keep its OpenCode and Pi wake protocol (#1988), reopened-session handling and
  OpenCode model selection; this branch's own plugin socket was dropped. Antigravity still reads its
  launch file from `agent_inbox`.
