---
asana_id: '1213718096138768'
linear_id: ce71ea4c-445b-4ad4-8048-d7ee7ca43346
notion_id: 32af8f99-3d81-81e6-9304-e51a78729af3
---
# API Expansion

**Finish line:** Concerto can inspect remote worktrees and drive typeahead/config UX through lfd HTTP APIs, without local filesystem assumptions.

## Context

The tend flow (scan-waves, assess) needs to read wave state — files, diffs, step/flow/direction metadata. Remote setups (Mac Mini dogfooding, team mode) can't assume local disk access. These endpoints also power Concerto's area picker and config editing UX.

## Scope

### In scope

- `GET /v0/waves/{wave_id}/files?path=`
- `GET /v0/waves/{wave_id}/file?path=`
- `GET /v0/waves/{wave_id}/diff`
- `GET /v0/steps?q=`
- `GET /v0/flows?q=`
- `GET /v0/directions?q=`

### Guardrails

- Every user path is resolved through `path_within_root_existing` / `path_within_root_planned`
- Reject traversal, absolute paths, symlink escapes, null bytes
- Enforce file-size caps on content reads
- Keep responses fast enough for interactive typeahead

### Out of scope

- Full IDE-grade remote code browser UX
- Arbitrary write/edit endpoints

## Done when

- Remote file/diff reads work for active wave worktrees
- Typeahead no longer requires local disk access in remote mode
- Security path-validation rules are enforced for every filesystem touchpoint
- Latency is acceptable for WAN usage
