# Open Questions

## Rust Core (lf-core)

- Flow loading supports YAML/JSON under `.lf/flows/` and only linear `Step` items in `tick_flow`; fork/choose/loop are parsed but not executed yet.
- `run_step` shells to `lf --step <name>`; if the CLI expects `lf <step>` or different flags, the runner should be updated.
- Which Python flow behaviors should be left behind vs matched exactly?
- How much of prompt rendering should be configurable vs hard-coded?
- Which tokenizer is acceptable, and when do we fall back to byte limits?

## Stacking (lfd next/rebase)

- Should `lfd next` and `lfops next` be unified or kept deliberately separate?
- Why does `lfops next` create new worktrees while `lfd next` reuses the same worktree?
- No tests for lfd stacking commands—is this intentional or an oversight?

## Assumptions

- Rust core is at `rust/lf-core` with a root Cargo workspace.
