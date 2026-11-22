# cfadc6 — One design system

## Exists (commit 9bc0fdecca)

- `DESIGN.md` replaces `VISUAL_DESIGN.md` and `swift/DESIGN.md`; no references to either old path remain outside `scratch/`.
- `design/prototypes/` holds the reference pages for the Mac app and website (`DESIGN.md:90`).
- `website/static/style.css` and `docs/dev/index.html` use named size/space/radius steps.
- `GhosttyManager.loopflowConfig` turns ligatures off, thickens strokes, adds 10% cell height.

## Checks

- Website suite: 73 passed (implement pass).
- Swift build of `GhosttyManager.swift`: not run here; gate/CI builds it.

## Remains

Nothing in this Task. Moving the Mac app and website code onto the `DESIGN.md` language belongs to the design Tasks stacked on this one.

`scratch/design-audit/` and `scratch/design-explore/` are exploration pages; several link to the copies of the prototypes kept beside them, so they stay until gate clears scratch.
