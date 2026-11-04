# Recovery and delivery semantics

Research for Jack Heart · November 3, 2026. Historical evidence, not new direction.

Work watches replace complete readings; store revisions are not replay cursors. Terminal recovery is owner/viewer/offset scoped and bounded; a fresh snapshot resets the renderer. Raw keys are never automatically resent. Semantic mutation recovery belongs in shared command admission and must report unknown effects honestly. Bound every queue; a slow phone must not stall execution.

Complete source, citations and contrary evidence: `9e4587bb53fc4102e8682a3bba61e9776a6de5c2:scratch/research-recovery.md` (`git show` that reference). HTML studies and image assets remain in this checkout. Current implementation and remaining proof: [mobile-ios.md](mobile-ios.md).
