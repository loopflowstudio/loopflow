---
linear_id: 220a9517-52fe-4cad-97dd-a764a1d9ddbd
---
# 09: Notion README sync

**Finish line:** a wave can link one canonical Notion page and round-trip it with `wave/<name>/README.md` through the existing PM lifecycle verbs.

The real reason Notion is interesting is not just another task backend. It is the first provider that can preserve wave intent as docs instead of flattening everything into tasks or issues.

## What to build

1. Add a wave-level pointer to one canonical Notion README page (new field in `WavePmConfig` or `NotionConfig`).
2. Pull that page into `wave/<name>/README.md` during `pm init` / `pm pull` using the existing `NotionClient` and `blocks_to_markdown`.
3. Push local README changes back to that page during `pm sync` using `markdown_to_blocks` and the block-replacement pattern from `update_item`.
4. The markdown↔blocks converter already exists in `pm/notion_blocks.rs` — reuse it directly.

## Constraints

- README linkage should be explicit, not guessed from a task database.
- This item is about one canonical page, not arbitrary doc trees.
- Hook into the existing PM lifecycle verbs rather than inventing a separate doc-sync command.
- The block-replacement pattern (delete all top-level blocks, re-append) means concurrent edits to the same page will conflict — document this.

## Done when

- A wave can point at a Notion README page
- `pm init` / `pm pull` refresh local `README.md` from that page
- `pm sync` can push `README.md` back
- The value of Notion as a doc-native source is proven on one page
