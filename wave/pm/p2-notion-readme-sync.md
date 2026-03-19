---
linear_id: 87d2f61c-37a7-4391-95fa-8a69217cc8ce
---
# 09: Notion README sync

**Finish line:** a wave can link one canonical Notion page and round-trip it with `wave/<name>/README.md` through the existing PM lifecycle verbs.

The real reason Notion is interesting is not just another task backend. It is the first provider that can preserve wave intent as docs instead of flattening everything into tasks or issues.

## What to build

1. Add a wave-level pointer to one canonical Notion README page.
2. Pull that page into `wave/<name>/README.md` during `pm init` / `pm pull`.
3. Push local README changes back to that page during `pm sync`.
4. Keep the first pass simple and lossy: plain block-to-markdown and markdown-to-block conversion is fine.

## Constraints

- README linkage should be explicit, not guessed from a task database.
- This item is about one canonical page, not arbitrary doc trees.
- Hook into the existing PM lifecycle verbs rather than inventing a separate doc-sync command.

## Done when

- A wave can point at a Notion README page
- `pm init` / `pm pull` refresh local `README.md` from that page
- `pm sync` can push `README.md` back
- The value of Notion as a doc-native source is proven on one page
