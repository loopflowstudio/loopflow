# 04: Real Tools (Track B2) — Shipped

Eleven tools across three tiers: boundary (`send_message`, `memory_edit`), context (read/write/delete/list with token counting), file/shell (ephemeral workspace with path traversal protection). Events ride on `ToolResult { output, event }` — boundary tools emit `AgentEvent`s, internal tools return `None`. JSONL output from `lf-agent`. Three-level registry: `default_registry()` (4) → `registry_with_context(store)` (8) → `full_registry(store, workspace)` (11).

## What we learned

`send_message` as a tool works cleanly — the model calls it naturally and the completion contract (`exactly one final`) validates via the same event stream. Context management is simple: `HashMap<String, String>` with approximate token counting (`cl100k_base` via tiktoken-rs) is sufficient for budget visibility. Ephemeral workspace isolation via tempdir + path canonicalization works — both relative (`../`) and absolute path traversal are caught. Constructor injection for tool state (`Arc<Mutex<ContextStore>>`, `PathBuf`) keeps the `Tool::call(&self, input)` signature clean without needing a shared context object. The compress pass after implementation was valuable — merging `registry.rs` into `tools.rs` removed a file without losing clarity.
