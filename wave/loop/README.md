# Loop Stack + PR Workflow

Make stacked wave iterations landable, observable, and recoverable.

## Vision

Stack lineage is explicit, GitHub live PR state is current truth, and queue advancement is deterministic (`Draft -> Ready -> Merged`) with exactly one Ready PR at a time.

## Goals

### Locked v1 decisions

- Keep stacked iterations and track ancestry explicitly.
- Make GitHub live PR state authoritative for current status.
- Create PRs as Draft by default.
- Keep exactly one Ready PR (oldest unmerged) at any time.
- Detect merges from any surface (GitHub/Concerto/CLI) and advance queue automatically.
- Rebase only the immediate next draft after each merge (lazy rebase).
- Keep **Combine PRs** as explicit escape hatch and model it in run history.
- Treat `scratch/` as working memory; publish review artifacts to PR-managed output.

### Done when (wave complete)

- Stack ancestry is explicit in storage and API.
- Open PR counts and badges match GitHub state, including out-of-band merges.
- Queue-head merge auto-advances exactly one next item or marks blocked on conflict.
- Combine PRs produces coherent run history with supersession links.
- Ready PRs are scratch-clean while review context remains visible on GitHub.

## Risks

### Plan adjustments from shipped work

- `queue_role` is projected, not historically persisted. Phase 03 must add durable combine event/supersession facts if we want explainable history.
- Queue fields (`queue_role`, `queue_block_reason`, `queue_blocked_at`, `next_action`) already exist in run DTOs. Phase 04 should consume, not redesign, backend queue semantics.
- Scratch-clean gating already runs during promotion. Phase 04 should focus on user-facing blocked remediation and review artifact publishing.
- Stale PR state is expected when no GitHub token is configured. Queue UX must treat this as degraded mode, not hard failure.
- Supersession is already partially modeled — `Superseded` status exists but is inferred from live state, not durable. Phase 03 converts inference to fact.
- `QueueBlockReason::FromStr` errors on unknown values. Phase 03 should add new reasons (e.g., `CombinePending`) to the enum before storing them, not rely on string tolerance.
- Implementation of 01+02 was larger than estimated (~2,300 LOC across 33 files vs. estimated 800-1,600 combined). Phase 03 estimate may similarly need headroom, especially for atomicity/retry paths.
- Per-wave reconcile locks and `QueueOps` trait are clean extension points — Phase 03 should add combine operations to the existing trait rather than building parallel infrastructure.
- `QueueOps` trait currently has five methods (checkout, mark_ready, mark_draft, rebase, scratch_clean). Phase 03 must add `close_pr` and `create_combined_pr` — the trait was designed for extension but these methods don't exist yet.
- Poll-based reconciliation runs on a 60-second interval. Phase 04 UX should account for this latency in poll-only setups; webhook path provides near-instant updates when configured.

### What might change

- If stale-state noise in no-token setups is too high, we may need stronger UI/system guardrails around queue confidence.
- Combine may require a stricter reconciliation state machine if GitHub operations and DB writes cannot be made sufficiently atomic.
- `QueueBlockReason` parsing is strict — adding new reasons requires coordinated binary + migration updates. If this becomes a pain point, consider a fallback `Unknown(String)` variant.

## Metrics

- Queue projection always yields exactly one Ready PR when unblocked
- Merge reconciliation advances or blocks deterministically with clear persisted reasons
- Superseded/combine states are explainable from durable run history, not only live inference
- Review artifacts remain visible on GitHub while promotion enforces scratch-clean readiness

