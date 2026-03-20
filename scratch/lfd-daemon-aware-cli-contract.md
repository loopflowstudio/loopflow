---
linear_id: b00f983a-47b2-4ea8-b357-e45e0d183aa3
---
# Wave-Aware CLI Runtime Journal

## Try it

```bash
cargo test runtime_run_keeps_worktree_clean --lib
cargo test lf_ops_land_writes_cd_directive_for_complete_rotation --test land_tests
cargo test --all
```

What you'll see:
- wave-attributed `lf` runs create `.lf/runtime/runs/<run_id>/meta.json` and `events.jsonl`
- `lfd` can replay those journals into `run.*` / `step.*` websocket events
- strict `lf ops land` no longer treats runtime-journal files as dirty worktree state
