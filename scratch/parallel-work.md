# LOO-298 control index

2026-09-30 · Kept by Jack Heart's supervising session. The previous index is in
history at `cbf01f5abk directed the current feature Flow for the remaining work: per item,
  implement → compress → rebase → realign → loop-decide. No slice or concept
  review. Delivery: publish → demo (Jack) → queue (compress, rebase, realign,
  gate) → `pr land -c`.
- One item per implement pass; focused tests plus hosted CI; the full local
  Rust suite only before the gate. Decisions reach the worker at boundaries.
- Plan: [remaining-work.md](remaining-work.md). Decisions: [questions.md](questions.md).

## State

- PR #1296. Hosted CI green on every behavior job at `b21f657fc`.
- Codex runs on loopflow-eng; the other three accounts are exhausted until
  October 3 to 6.
- LOO-334 is stacked on this branch and runs in parallel on the same account.
- Workers run the pinned lf 0.12.23 against the local installation's Home until
  this branch lands; LOO-334 fixes that.
