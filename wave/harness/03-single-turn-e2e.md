# 03: A Single Turn, End to End (Track B1) — Shipped

Turn loop calls Anthropic Messages API, dispatches tool calls via `ToolRegistry`, feeds results back, loops until text response or limit. `lf-agent` binary runs prompts from the CLI. Guardrails (max iterations, timeout) from day one.

## What we learned

The turn loop is straightforward — async for the API call, sync tool dispatch within the loop. The foundation contract types (`AgentEvent`, `ChatTurnResult`, completion validation) fit cleanly as the event vocabulary. Adding a new tool takes ~30 LOC (implement `Tool` trait, register it). The `ToolResult { output, event }` design — where boundary tools emit events and internal tools return `None` — keeps the registry generic.
