# `lf op cloud` — open questions (A2 increment 1)

No detailed design doc existed for the `.mcp.json` shape or deep-link, so I made
executive calls. Flag for review:

- **Asana MCP shape.** Emitting a hosted remote MCP entry (`type: sse`,
  `https://mcp.asana.com/sse`) with the wave's stored OAuth token as a
  `Bearer` header. The token is a live secret, so `.mcp.json` is added to
  `.git/info/exclude`. Open: is the hosted SSE server the right transport, or
  do we want a token-scoped stdio server? Workspace/project GIDs currently
  travel in the Goal prompt (the roadmap handle), not in the MCP config — the
  token grants workspace access and the agent uses the project GID from the
  prompt.

- **Deep-link target.** Claude has no create-routine API/deep-link, so the
  scaffold prints `https://claude.ai/new` and instructs the human to attach the
  repo, paste the prompt, and set a schedule. If there's a better prefill URL,
  swap `CLAUDE_CLOUD_URL` in `ops/cloud.rs`.

- **Codex is deferred.** `lf op cloud codex` returns a clear "follow-on"
  error. Codex reads MCP from `~/.codex/config.toml` (not `.mcp.json`) and has
  no server-side schedule, so its scaffold differs enough to be its own
  increment.
