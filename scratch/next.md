# Slice 3: make Wave memory file-only

## Implement

- Make `wave/<name>/MEMORY.md` the only Wave memory truth.
- Delete memory-added/updated journal variants, runtime folds/state/broadcasts,
  memory facts, replay, HTTP writes, SSE frames, receipts, and Swift mirrors.
- Delete `lf memory add|log|update`, hidden aliases, and the export-memory skill.
  Keep exactly `lf memory show` as a direct read that works with all servers
  stopped.
- Assemble Project/Task memory directly from applicable ancestor Wave
  `MEMORY.md` files, oldest-first. Do not read journal/live deltas.
- Remove Doctor, cron, prompt, docs, golden, and builtin references to live
  memory curation.

## Preserve

- `GOAL.md` and `MEMORY.md` as authored/durable Wave files.
- Generic mutation result types named `*Receipt`; evidence Receipt deletion is
  a later slice.
- Wave conversation itself; ambient transcript injection is the next slice.

## Done when

- [ ] journal/runtime/server/SSE/Swift memory facts and writes are absent.
- [ ] CLI exposes exactly `lf memory show`, usable without a server.
- [ ] prompt memory reads only ancestor `MEMORY.md` files oldest-first.
- [ ] export-memory and live-curation docs/prompts/tests are absent.
- [ ] focused memory/journal/runtime/prompt/CLI/Swift proofs, fmt, and clippy
      pass.
