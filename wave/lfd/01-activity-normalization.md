# 03: Activity Normalization

**Finish line:** lfd emits typed activity events (shell, read, edit, write, search) as agents stream output, persists them to the database, and broadcasts them to clients over the existing WebSocket.

## Context

Agent output is currently raw text. Concerto (desktop and mobile) wants to render "agent edited src/api/routes.rs" differently than "agent ran cargo test." That requires a canonical typed event model for tool calls, normalized across providers.

This is the "structured observation" described in the lfd wave README: `lf` reports structured events, lfd persists and fans out.

## What to build

**Parser in lfd.** As agent output streams through lfd, parse tool calls into typed events. Each provider (Claude Code, Codex, OpenCode) emits tool calls differently — the parser normalizes them into a canonical schema.

**Starting schema:**

```rust
enum ActivityEvent {
    Shell { command: String, exit_code: Option<i32>, cwd: Option<String> },
    FileRead { path: String },
    FileEdit { path: String, diff: Option<String> },
    FileWrite { path: String },
    Search { query: String, path: Option<String> },
    Unknown { tool_name: String, raw: String },
}
```

Start small. `Unknown` is the escape hatch — anything we don't recognize yet still gets captured. Add variants as needed.

**Persistence.** Normalized events go into the database as part of the run/session event stream. Raw log files remain unnormalized.

**Broadcast.** Typed events are emitted over the existing WebSocket alongside `OutputLine` events. Clients that understand them get structured rendering. Clients that don't still get raw output.

## Constraints

- Parse in real-time as output streams, not post-hoc
- Don't block or slow down output delivery — parsing failures should fall through to `Unknown`, never drop events
- Provider-specific parsing details (Claude's tool names vs Codex's) stay in the parser, not in the schema
- Schema must be forward-compatible — adding a new variant shouldn't break existing clients

## Done when

- lfd emits typed activity events for shell commands and file operations during a Claude Code session
- Events are persisted to the database with run/session correlation
- Events appear on the WebSocket event stream
- Concerto desktop can render at least shell and file-edit events with structured UI (command text, file path, diff preview)
