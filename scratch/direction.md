# Direction (LOO-974)

2026-11-18, Jack Heart on `AgentExecutionBoundary` / `AgentWriteScope` / `checkout_execution_boundary` / `supports_execution_boundary` after seeing that `writable_roots` is unread and `kickoff`'s plan deletes the boundary machinery instead of enforcing checkout write denial:

- "that is a weird thing to have built"
- "sounds good lets continue"
- "noting that there is an API opencode run that may be the right way to run oepncode headlesss"
- "did we move off the codex server" / "that is seemingly simiilar to opencode serve"
- "yeah lets do that"
- "Nah, bring that into this task"

Scope folded into LOO-974: align OpenCode launches with the split Claude and Codex use — `opencode --standalone` directly in the terminal for bare chats (no companion thread), `opencode run --standalone --format json --auto` for headless batches and Task workers, and `opencode serve` + `--server` kept only if `MachineServer` uses it to wake idle OpenCode TUI work sessions on steers (otherwise deleted too), removing obsolete `opencode serve` HTTP/SSE machinery.
