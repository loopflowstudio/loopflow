# Portfolio tiers in Concerto

Shipped. Design folded into `wave/workflows/2-governance-surfaces.md` (portfolio
section). What remains here is how to validate the change.

## Validate

```bash
swift test --package-path swift --filter Portfolio   # legacy decode, reorder math, sort key
swift test --package-path swift                       # full Swift package
```

## Manual QA (needs a rendering environment — not run headless)

1. Launch Concerto. Existing portfolio repos appear under **Active** (legacy
   migration); nothing is lost.
2. Drag Cadenza and Loopflow into **Core**, drag Studio into **Deprecated**,
   reorder Cadenza above Loopflow within Core.
3. Quit and relaunch — tier placement and within-tier order persist.
4. Sections render in fixed order Core → Active → Future → Deprecated with
   burgundy headers; empty tiers still show as drop targets.
