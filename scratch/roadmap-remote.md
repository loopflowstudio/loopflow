# Asana is the roadmap: remove the mirror, the abstraction, and ingestion

Consolidates the intent of PRs #726 (asana-only) and #758 (live roadmap handle),
reconciled to the current model: **Asana is the single source of truth for a
wave's roadmap, reached only through `lf op pm`.**

## What changed

- **Asana-only.** Deleted `lfd/pm/linear.rs`, `notion.rs`, `notion_blocks.rs`,
  the `PmProviderKind` enum, and the `PmProvider` trait. `AsanaClient` is a
  concrete client (its former trait methods are now inherent). Config drops
  `LinearConfig`/`NotionConfig`/`PmRolesConfig`; wave frontmatter is just
  `pm.asana_project`.
- **No local roadmap mirror.** Removed `pm pull/export/push-diff/sync/import` and
  all the on-disk `wave/<name>/N-*.md` machinery from `ops/pm.rs`.
- **New `lf op pm` API** (talks to Asana directly):
  - `lf op pm show [--wave w]` — print the live roadmap
  - `lf op pm update [--wave w] [--id t] --title … [--notes …] [--status done]`
  - `lf op pm init [--wave w]` — connect/create the project, write `asana_project`
  - `lf op pm status` — open/total per linked wave
- **Ingestion removed.** No `ingest` step, no `lf op ingest`, no auto-pick leg in
  `build-or-silent` / `govern-operations`. Workers are handed their task at
  dispatch. The immediate-activation `roadmap_item` is now a logged hint, not a
  file ingest.
- **Prompts/docs.** `update-wave`, `design`, `scan`, `split-wave`, `kickoff`,
  `implement`, `LOOPFLOW.md`, and `docs/*` now describe GOAL.md + MEMORY.md +
  remote Asana roadmap. `GOAL.md` replaced `README.md` as the wave anchor.

Net ≈ −7,700 / +960 lines.

## Try it

```bash
# Rust
cd rust/loopflow
cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
cargo run -q --bin lf -- op pm --help          # show / update / init / status

# Python + Swift (mirrors)
uv run pytest python/tests/ -q
swift test --package-path swift
```

`lf op pm show` needs a wave whose `GOAL.md` has `pm.asana_project` and a live
Asana credential (`lf op auth asana`).

## Follow-ups (not in this branch)

- **Local `wave/*/N-*.md` files are now orphaned.** They hold real planning
  content and were left in place — nothing reads them anymore. Migrate each to
  the wave's Asana project (`lf op pm update`) then delete, or delete outright.
  A human/live-auth call, not a headless one.
- **`roadmap_item` hint → Asana fetch.** Immediate activation logs the hint but
  no longer materializes the task into `scratch/`. If "build this specific item"
  should still seed the worker, fetch the Asana task by id and write it to
  `scratch/` at activation.
- **Swift ingest UI.** `MultiplexerView`'s roadmap "play/Return → ingest & build"
  affordance and `RepoState.ingestAndBuild` still exist (they dispatch a build
  flow with a roadmap-item override — not the removed auto-pick). Decide the
  roadmap row's primary action if that surface stays.
