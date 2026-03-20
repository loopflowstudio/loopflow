---
linear_id: 2b2f8d4f-ef39-42a5-b8ba-36ebf60b87b7
---
# 10: Notion supporting docs import

**Finish line:** after README sync works, a wave can pull the adjacent Notion docs it actually needs without flattening them into tasks/issues.

README sync proves the core shape. The next step is bringing in the nearby supporting pages that make a wave useful: architecture notes, rollout docs, product context, and other non-task material.

## What to build

1. Decide how a wave points at the supporting pages to import: explicit links, child pages, or a small rooted subtree.
2. Pull those pages into durable local docs next to the wave.
3. Keep overwrite behavior explicit so imported docs do not surprise users.
4. Stay narrow: import the pages that matter to the wave, not a whole workspace crawler.

## Constraints

- Build on top of the canonical README page link from item 09.
- Keep naming and overwrite rules obvious.
- Do not turn this into arbitrary workspace sync.

## Done when

- A wave can import its nearby supporting docs from Notion
- Those docs land in predictable local files
- The feature still feels wave-scoped, not workspace-wide
