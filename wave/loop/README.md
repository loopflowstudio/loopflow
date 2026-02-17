# Loop Stack + PR Workflow

Make stacked wave iterations landable, observable, and recoverable.

## Scope

This wave focuses on the implementation path from today's implicit stacking to a deterministic queue-based PR workflow.

## Locked v1 decisions

- Keep stacked iterations and track ancestry explicitly.
- Make GitHub live PR state authoritative for current status.
- Create PRs as Draft by default.
- Keep exactly one Ready PR (oldest unmerged) at any time.
- Detect merges from any surface (GitHub/Concerto/CLI) and advance queue automatically.
- Rebase only the immediate next draft after each merge (lazy rebase).
- Keep **Combine PRs** as explicit escape hatch and model it in run history.
- Treat `scratch/` as working memory; publish review artifacts to PR-managed output.

## Sequence (sized for implementation)

Each item is scoped to ~100-1000 LOC of implementation work.

| # | Item | Focus | Est. impl LOC |
|---|------|-------|---------------|
| 01 | Foundations + Live State | Explicit stack model and live PR sync | 350-700 |
| 02 | Queue Lifecycle + Merge Advancement | Draft/Ready invariant and lazy rebase flow | 450-900 |
| 03 | Combine PRs Reconciliation | Modeled combine event + run reconciliation | 250-600 |
| 04 | Queue UX + Review Artifacts | Queue-first Concerto view + scratch-safe PR publishing | 350-800 |

## Deliverables

- `wave/loop/01-foundations-live-state.md`
- `wave/loop/02-queue-lifecycle-merge-advancement.md`
- `wave/loop/03-combine-prs-reconciliation.md`
- `wave/loop/04-queue-ux-review-artifacts.md`
- `scratch/loop-stack-pr-workflow-foundations.md` (implementation-ready foundation spec)

## Done when

- Stack ancestry is explicit in storage and API.
- Open PR counts and badges match GitHub state, including out-of-band merges.
- Queue-head merge auto-advances exactly one next item or marks blocked on conflict.
- Combine PRs produces coherent run history with supersession links.
- Ready PRs are scratch-clean while review context remains visible on GitHub.
