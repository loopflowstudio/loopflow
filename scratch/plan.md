# cfadc6 — One design system

## Exists (commit 9bc0fdecca)

- `DESIGN.md` replaces `VISUAL_DESIGN.md` and `swift/DESIGN.md`; no references to either old path remain outside `scratch/`.
- `design/prototypes/` holds the reference pages for the Mac app and website (`DESIGN.md:90`).
- `website/static/style.css` and `docs/dev/index.html` use named size/space/radius steps.
- `GhosttyManager.loopflowConfig` turns ligatures off, thickens strokes, adds 10% cell height.

## Checks

- Website suite: 73 passed (after the QA nav fix and regenerating `docs/dev/index.html`).
- QA (`scratch/qa.html`): 14 pages at desktop and phone widths, copy button, nav menu, `docs/dev/index.html`; Ghostty `+validate-config` accepts the new terminal defaults.
- Swift build of `GhosttyManager.swift` and a look at the terminal rendering: not run here; gate/CI builds it, a person looks.

## Remains (from concept review, `scratch/concept-review.html`)

The Task's outcome is one design system, but the site still serves a second one. Do these in this Task:

1. Delete `website/internal_pages.py`, the `/fonts`, `/colors` and `/design` routes and their import in `website/main.py`, and its entry in `website/pyproject.toml`. These pages present an older system (Cormorant, Lato) that nothing links to and that contradicts `DESIGN.md`. This is a choice made for review; git keeps them. Then run the website suite.
2. Move `design/homepage-next/` to `design/prototypes/homepage-next/` so prototypes have one home. Leave `release/v0.15.3/NOTES.md` as written (shipped record). Update the `design/prototypes/` line in `DESIGN.md` to list it.
3. Move `DESIGN.md § Work tracking` (sidebar behaviour, not look) into `swift/README.md`.

Left to the stacked website Task: apply and delete `design/prototypes/website-fonts.patch`, and rename the web `--t-*` steps to the language's eight names (rule 4). That rename changes sizes on the page.

Beyond that, moving the Mac app and website code onto the `DESIGN.md` language belongs to the design Tasks stacked on this one.

`scratch/design-audit/` and `scratch/design-explore/` are exploration pages; they link into `design/prototypes/` and stay until gate clears scratch.
