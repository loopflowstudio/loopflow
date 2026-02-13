# 02: Persistence + Token-Bounded History

Implement durable memory and token-based harness history retrieval.

## What exists after this

- tables/repository methods for memory blocks and message log
- per-edit memory durability
- history fetch by token budget (not turn count)

## Commit slices

### C1 — Add schema + migrations (~250-400 LOC)

- `chat_memory_blocks` and `chat_messages`
- indexes by `wave_id`, `created_at`
- optional `token_count` column for faster budget queries

### C2 — Repository methods (~300-500 LOC)

- `load_memory(wave_id)`
- `apply_memory_edit(wave_id, op, ...)` (persist each edit)
- `append_chat_message(wave_id, role, content, phase, metadata)`
- `load_history_by_tokens(wave_id, token_budget)`

### C3 — Persistence tests (~250-450 LOC)

- per-edit memory durability tests
- token budget boundary behavior
- ordering and truncation correctness

## Constraints

- Memory persistence is immediate per memory tool call.
- History loading must be deterministic and token-budget-driven.
- Keep persistence API independent from specific model provider.

## Done when

```bash
cargo test -p loopflow chat_store
```

Expected: schema + repository tests pass.
