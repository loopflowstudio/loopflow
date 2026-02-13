# 07: Python Local Client

Provide a thin Python API surface for fast local validation.

## What exists after this

- `chat_send`, `chat_memory`, `chat_memory_edit`, `chat_compact`
- typed response objects including progress/final traces
- simple local smoke usage from Python

## Commit slices

### C1 — API methods + models (~250-400 LOC)

- add chat client methods in `python/loopflow/api.py` + models
- preserve full payload fields needed for debugging

### C2 — Python tests + smoke snippet (~250-400 LOC)

- client behavior tests (success/error handling)
- local smoke example aligned with roadmap done-bar

## Constraints

- Keep client thin: pass through server behavior, minimal policy logic.
- No REPL in first pass.

## Done when

```bash
uv run pytest python/tests/ -k chat
```

Expected: chat client tests pass.
