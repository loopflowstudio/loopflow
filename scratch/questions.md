# Open choices made for review (dd8997 kickoff, 2026-11-21)

- The workflow card's `<small>` under `feature` says "The default.", but a
  Task with no Graph configured runs `code`. Copy is out of scope, so it stays.
- The shared bar's Install pointed at `/#start`, which no element on the
  homepage carries. It now goes to `/download` on every page.
- The docs' grey ground (`--d-paper #f6f6f4`) becomes the homepage's white
  `--bg`. Keeping a docs ground would need a new `:root` role.
- Docs section headings (sans 600) move from 27px to `--t-head` 24px rather
  than adding a step.
- Footers stay as they are: homepage footer and docs `SiteFooter` differ in
  copy, and merging them is a copy decision.
- The site has no dark scheme; "dark" checks mean OS dark mode leaves the
  light site intact. Adding one is a separate design.
