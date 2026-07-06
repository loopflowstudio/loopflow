# memory: lf memory log + recency injection (additive to main)

Targets **main** (the stacked parent already merged as a different, reworked
design — see the stacking TODO). Main already made `add` a pure stream publish
and kept `lf memory update` as the compiled-checkpoint writer (`MemoryUpdated`
resets the add-delta). This slice adds the two things main lacks.

**Remove nothing.** Keep `lf memory update`, `MemoryUpdated`, `MemoryAdded`, and
`append_memory` exactly as main has them. This is purely additive.

## What to build

1. **`lf memory log`** — a one-shot dump of the add-stream (the facts published
   since the last checkpoint), oldest→newest, one per line. Mirrors
   `lf memory show` (which prints the compiled `MEMORY.md` base). Read-only:
   no wave context is an error, like `show`.
2. **Recency injection** — `<lf:wave-memory>` becomes the recent add-stream facts
   layered *above* the `MEMORY.md` base, instead of the base alone. The mind
   reads recent-on-base, freshest first. Because `MemoryUpdated` resets the
   add-delta, "recent" is naturally bounded to adds-since-last-checkpoint.

A known-good implementation of both features exists at commit `27e29dff` (in this
branch's reflog). **Reuse that code** for `lf memory log` and the
`wave_context::gather_wave_memory` / `render_wave_memory` layering — but that
commit was built on a branch that *removed* `update`; graft only these two
additive features onto main's files, which keep `update`.

## The change

- **`lf memory log`**: add `MemoryCommand::Log { target }` (`lf/mod.rs`); a `log`
  handler in `lf/commands/memory.rs` that reads `GET /memory/log` when a server
  is live and folds the journal's `MemoryAdded` events offline; a
  `memory_log_handler` + `MemoryLogBody { facts }` + `GET /memory/log` route in
  `wave/server.rs`. The server reads the runtime's add buffer.
- **Injection**: add `gather_wave_memory(repo_root, wave)` to
  `engine/wave_context.rs` — reads the `MEMORY.md` base + the recent adds (live
  `GET /memory/log`, else journal fold) and renders recent-above-base (recent
  block newest-first for prompt recency; `lf memory log` stays chronological).
  Point `engine/prompt.rs::gather_wave_memory_doc` at it instead of the
  base-only `Memory::read()`.

## Ridealong

A `TODO(stacking)` comment lands near the worktree/land rotation noting the
broken case this branch hit: a stacked child is stranded when its parent is
*reworked* during land (merged content ≠ what the child stacked on) — the
rotation replays already-merged commits into conflicts. For now, stacked
worktrees just target main; the real fix (identity-preserving land for stacked
parents, or detect-rework-and-re-derive) is unresolved. See the systems wave.

## Demo

`lf memory add "workers report via lf chat"` → `lf memory log` prints the fact;
`MEMORY.md` unchanged; and the next run's `<lf:wave-memory>` shows the fact
*above* the compiled base. `lf memory update` still rewrites the base and clears
the delta (so `log` empties after a checkpoint).

## Done when

```bash
cargo test -p loopflow          # lf memory log (server + offline); injection
                                # layering test; goldens regenerated
cargo clippy -- -D warnings && cargo fmt --check
```

Checklist: `update`/`MemoryUpdated` untouched; `lf memory log` added with a test;
`gather_wave_memory` layers recent-above-base with a test; goldens updated for the
new injection shape; `TODO(stacking)` comment present.
