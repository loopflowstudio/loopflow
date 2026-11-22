# cfadc6 — One design system

## Exists (commit 9bc0fdecca)

- `DESIGN.md` replaces `VISUAL_DESIGN.md` and `swift/DESIGN.md`; no references to either old path remain outside `scratch/`.
- `design/prototypes/` holds the reference pages for the Mac app and website (`DESIGN.md:90`).
- `website/static/style.css` and `docs/dev/index.html` use named size/space/radius steps.
- `GhosttyManager.loopflowConfig` turns ligatures off, thickens strokes, adds 10% cell height.

## Checks

- Website suite: 73 passed (after the QA nav fix and regenerating `docs/dev/index.html`).
- QA (`scratch/qa.html`): 14 pages at desktop and phone widths, copy button, nav menu, `docs/dev/index.html`; Ghostty `+validate-config` accepts the new terminal defaults.
- QA after the cleanup (`scratch/qa.html` §5): 84 crawled pages answer 200, the three deleted routes 404 and leave the sitemap and llms.txt, and the Install button's focus ring is dark on wine.
- Swift build of `GhosttyManager.swift` and a look at the terminal rendering: not run here; gate/CI builds it, a person looks.

## Concept-review cleanup (done)

- `website/internal_pages.py` and the `/fonts`, `/colors`, `/design` routes are deleted; the site serves one design system.
- `design/homepage-next/` moved to `design/prototypes/homepage-next/`, listed in `DESIGN.md`; `release/v0.15.3/NOTES.md` keeps the old path (shipped record).
- `DESIGN.md § Work tracking` moved to `swift/README.md` before `## Code map`.
- Website suite after the deletion and the dead-selector cut (`.btn-secondary`, `.btn-group`, `.page-hero`, `.next-steps*`): 73 passed; `ruff check website` clean. `documented_commands::` (DESIGN.md and swift/README.md edits, no command changes): gate runs it.

## Remains

Left to the stacked website Task: apply and delete `design/prototypes/website-fonts.patch`, and rename the web `--t-*` steps to the language's eight names (rule 4). That rename changes sizes on the page.

Beyond that, moving the Mac app and website code onto the `DESIGN.md` language belongs to the design Tasks stacked on this one.

`scratch/design-audit/` and `scratch/design-explore/` are exploration pages; they link into `design/prototypes/` and stay until gate clears scratch.

## Ship review (headless)

`scratch/ship-review.html`; reading order in `scratch/tour.yaml`. No new Task. The two pre-existing QA nits (light-blue inner focus ring, bare 404) were steered into dd8997. Gate has to see `macos` and `website-test` run: CI skipped them on the scratch-only head.
