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

# Compress (2026-11-21)

- `/favicon.ico` answers 404: fast_app's `/{fname}.{ext:static}` route
  catches it first and looks for `./favicon.ico`, so main.py's favicon
  handler never runs. Browsers use the `<link rel="icon">`; left alone.

# QA (2026-11-21)

- Pico loads from `cdn.jsdelivr.net/npm/@picocss/pico@latest`, unpinned, on
  every page. Pinning a version or dropping Pico (`pico=False`) is a separate
  change; most of the site's resets exist to undo Pico.
- The 404 page's words ("Page not found", Home · Docs) are new copy, kept to
  the minimum a site page needs.
- The /download copy button now shares the homepage's `data-copy` script, so it
  says "Copied" on click; its icon returns after 1.6s.

- Kept this Task’s plan and review pages through the design branch’s scratch cleanup. The shipped stylesheet takes its dark keyboard focus ring for solid buttons.

# Ship review (2026-11-21, headless)

- No one was present; no reactions recorded. Questions 1–4 on
  `scratch/ship-review.html` are open.
- Filed Task d5e6d7 (drop unpinned Pico and its resets). The favicon route
  stays with gate, as the plan says.
