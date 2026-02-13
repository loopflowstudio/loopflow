# 08: Swift Client + Viewer UI

Ship the first product-facing chat surface: streamed messages and memory viewer.

## What exists after this

- Swift client calls chat endpoints
- UI shows live `progress` and `final` messages
- memory blocks are viewable in UI (no editor yet)

## Commit slices

### C1 — Swift chat API client (~250-450 LOC)

- request/response models
- event stream parsing
- error mapping for partial/failed runs

### C2 — Chat transcript UI (~300-550 LOC)

- render progress vs final messages clearly
- include graceful failure state when final message is missing

### C3 — Memory viewer UI (~250-450 LOC)

- block list + details view
- refresh after successful turns

## Constraints

- UI remains thin; no hidden business logic.
- Distinguish progress vs final as explicit message phases.
- Viewer only in first pass (no memory editing controls).

## Done when

```bash
swift test --package-path swift
```

Expected: Swift client/UI tests pass.
