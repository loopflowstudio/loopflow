# GOAL.md as charter

Finish making GOAL.md + projects/ the wave's whole durable identity.
Audited 2026-07-08: `primary_flow` is fully retired in live code (migration
053; zero references) — that KR is done and dropped. The single-repo Wave
model landed in Swift.

## KRs

- Swift flow vocabulary finishes: delete the orphaned `Flow.swift` (zero
  references), delete the legacy `Trigger.flow` path, and adjudicate the
  retained flow-named fields (`Run.flow`, `flowSteps`, `WaveCron.flow`,
  Catalog flow types) against the settled nouns — most are correct and
  stay (Linear 07d58fc0).
- Journal engineering keeps the record honest at scale: rotation,
  segmentation, compaction — more timely post-#845's journal reset
  (ff593217).
- Stale generated surfaces regenerate: `.lf/summary.md` still shows
  pre-rename shapes.
