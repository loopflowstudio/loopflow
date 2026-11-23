# Questions (LOO-1002 kickoff, made for review)

- Pi wakes with `pi.sendMessage({customType: "loopflow-steer", …}, {deliverAs: "followUp", triggerTurn: true})`
  rather than `sendUserMessage`: the transcript shows lf's message as lf's, as Claude's
  `<cross-session-message from="loopflow">` envelope does. Passed over: a user message,
  which Pi renders as typed by the person.
- Plugin wake sockets live under `<LF_HOME>/wake/<random>.sock`, chosen by the plugin and
  reported in the boundary's existing `inbox` field. Passed over: an lf-chosen path in the
  launch spec, which would break the content-addressed Pi extension file in `hook-settings/`.
  A crashed provider leaves its socket file; the server only ever connects to the one recorded
  for a live AgentProcess, so leftovers are files, not behavior.
- Antigravity is in scope (Task comment, 2026-11-23: "Scope includes Antigravity"). Its wake uses
  `agy`'s undocumented language-server RPC with a launch-set `ANTIGRAVITY_SERVER_CSRF_TOKEN`.
  Passed over: `agy acp`, which would replace the TUI instead of waking it; `--remote-control`,
  which relays through Google and needs a login; the `Stop` hook's `continue`, which works only
  as a turn ends and never while idle.
- OpenCode wakes from its own plugin (`session.synthetic`), not by posting to the TUI's
  `serve --stdio` child: that child's port and password are published nowhere. No lf-owned
  OpenCode server returns.
