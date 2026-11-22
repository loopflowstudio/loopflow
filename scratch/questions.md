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

# Concept review (2026-11-21)

- One footer on every page, the homepage's. The docs/download footer
  ("Loopflow, a software instrument." / "Built by Loopflow Studio") goes,
  and studio pages, which had none, gain one. Reversible; existing copy only.
- The workflow card's `code` line will show two highlighted stops
  (human-review, verify) under `<small>` copy that begins "One stop." The
  copy stays word for word; it now contradicts the line beside it.

# Implement (2026-11-21)

- `homepage.install.links` ("Getting started ↗", "Source ↗") was read by no
  page, so the merged top-level `install` block drops it instead of carrying it.
- Docs and download lose the old bar's `target="_blank"` on GitHub; the shared
  bar opens it in the same tab, as the homepage always did.
- fast_app loads Pico before `style.css`; the bar's `details`/`summary` rules
  undo Pico's chevron and margins. Turning Pico off (`pico=False`) would delete
  many such resets across the stylesheet, but changes every page's base styles.

## Sync with the design Task

- The shared header takes the open Company menu fix. The consolidated font
  prototype goes because its faces already ship. This Task keeps its type
  step names; the upstream suggestion to rename the ramp remains open.
