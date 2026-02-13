# 03: lf-agent Skeleton

Create the new Rust agent binary with model/tool abstraction and guardrailed turn runner skeleton.

## What exists after this

- `rust/lf-agent/` crate in workspace
- core traits (`Model`, `ToolHandler`, `ToolRegistry`)
- turn runner skeleton with iteration + timeout guardrails
- no real provider/tool implementations yet (stubs only)

## Commit slices

### C1 — Create crate + domain modules (~250-450 LOC)

- `main.rs`, `loop.rs`, `messages.rs`, `events.rs`, `memory.rs`
- wire workspace dependencies (`tokio`, `serde`, `reqwest`, `async-trait`, `anyhow`)

### C2 — Add core traits and registry (~250-450 LOC)

- `Model` trait and response types
- `ToolHandler` trait + registry dispatch
- stub tool and model implementations for compile-time wiring

### C3 — Add guarded turn loop skeleton (~300-500 LOC)

- loop with max iteration and wall-clock timeout checks
- accumulation of message/tool call log
- explicit completion hook waiting for final message flag

## Constraints

- Keep provider abstraction clean (Anthropic first, extensible later).
- Guardrails required from day one.
- Event emission interface must be JSONL-friendly.

## Done when

```bash
cargo test -p lf-agent
```

Expected: crate builds and unit tests pass.
