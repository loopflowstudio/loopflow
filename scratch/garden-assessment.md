# Product Wave Assessment — 2026-07-16

## Summary

**Health: steady, proof-poor.** Product has closed 24 of 36 tasks and the five
reviewed PRs now sit on main, but Linear marks 0 of 14 KRs as holding. The gap is
not shallow work; it is that the KRs demand week/month streaks and complete
cross-surface trials after the prerequisite slices land.

The operating surface is currently weaker than the implementation queue:
`lf status product` and the Product roadmap cannot render the hierarchy because
a live `wave-chat` Project Session points at a Project absent from the refreshed
PM snapshot. The registry reports four active Projects while Linear now exposes
three. Product is idle (`live=false`) with 12 active tasks.

## KR scorecard

| Project | Official proof | Current trajectory | Missing evidence |
|---|---:|---|---|
| Mac Surface UX | **0/5 hold** | Attention and Active Sessions foundations are closed. #998 and #990 are merged. | No one-week surface run, 20-row lens sample, 20 cold-open/send trial, 10/10 handoff trial, or one-week Active Sessions census. PRD-1 and PRD-4 remain open contract gaps. |
| Loopflow API | **0/5 hold** | Recovery, supervision, CI-fix, and OpenCode failure work attack the correct reliability boundary. #980 and #1020 are merged. | No five-wave/week load streak, unattended-task-loop week, spawn-chain budget week, one-model month, or cold 3/3 lifecycle proof. The stale Project-session mismatch and manual branch rescue are current counter-evidence. |
| Auditability | **0/4 hold** | Shared state reasons landed; #974's Task→run→trace identity join is merged after passing the full resolver matrix. | `lf status` and roadmap are unavailable for Product now. No week of surface-only answers/drill-down, full-lifetime state-reason proof, or month of claim receipts. PRD-11 remains unstarted. |

## Landing evidence

- [PRD-2](https://linear.app/loopflow/issue/PRD-2/open-interactive-handoffs-in-the-last-successful-surface) — `handoff-real-surface-proof` — #998 merged at `6e18861e52`.
- [PRD-3](https://linear.app/loopflow/issue/PRD-3/make-wave-chat-fast-durable-and-legible-across-failures) — `wave-chat-durable-delivery` — #990 merged at `7f72eaf050`.
- [PRD-5](https://linear.app/loopflow/issue/PRD-5/make-opencode-glm-sse-disconnects-observable-and-recoverable) — `make-opencode-glm-sse-disconnects` — #1020 merged at `a0cb0c65f`.
- [PRD-9](https://linear.app/loopflow/issue/PRD-9/recover-an-abandoned-task-without-losing-its-work-or-pr-history) — `2` — #980 merged at `c78534a3da`.
- [PRD-12](https://linear.app/loopflow/issue/PRD-12/let-users-drill-from-roadmap-intent-to-live-work-and-complete-trace) — `let-users-drill-from-roadmap` — #974 merged at `6b659d92be`.

The shared roadmap could not supply `next_move.owner` for these rows because the
stale `wave-chat` Project Session makes the Project hierarchy unavailable.

## Momentum and drift

**Velocity:** Five substantive Product PRs reached review and merged in one day.
The queue ran every merge-group gate against the evolving main branch.

**Depth:** Strong. The work targets atomic Chat delivery, exact Session attach,
observable provider failure, abandoned-work recovery, and durable trace joins.
Those are direct KR prerequisites rather than cosmetic output.

**Alignment:** Strong at the task level. Drift appears in portfolio/runtime
coherence: the charter memory names seven live bets, the refreshed initiative
contains three, and the runtime still supervises the removed `wave-chat`
Project.

**Health:** Mixed. Failures were actionable and named, and the landing queue is
clear. The wave nevertheless cannot answer its own status or roadmap query.

## Pressure Points

1. **Capability-to-proof conversion:** zero KR clocks are running despite 24
   closed tasks; the wave has no accepted week/month evidence window.
2. **Portfolio/runtime coherence:** the stale `wave-chat` Project Session makes
   Product's own status and roadmap unavailable and exposes a 4-vs-3 Project
   split.
3. **Proof activation:** the newly landed handoff, Chat, recovery, provider, and
   drill-down capabilities still need the sampled and streak evidence their KRs
   require.
