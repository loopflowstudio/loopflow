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

## Remains

Nothing in this Task. Moving the Mac app and website code onto the `DESIGN.md` language belongs to the design Tasks stacked on this one.

`scratch/design-audit/` and `scratch/design-explore/` are exploration pages; they link into `design/prototypes/` and stay until gate clears scratch.
