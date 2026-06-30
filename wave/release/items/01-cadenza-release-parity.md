---
priority: p1
status: open
---
# Cadenza release parity

Mirror Loopflow's release automation shape in Cadenza.

## Goal

Cadenza has the same daily verification and weekly release cadence as Loopflow, with product-specific build/test commands documented rather than improvised.

## Done when

- Cadenza nightly workflow verifies release-grade artifacts without publishing.
- Cadenza weekly workflow publishes only after same-run verification passes.
- Local update script is one command and documented.
- Any difference from Loopflow's release schedule or deploy shape is intentional and written down.
- PR body includes a Mitchell Hashimoto simulated review.
