# LOO-672 — Accepted decisions

Jack Heart selected:

- October 28, 2026: return at the bottom; exact scroll position need not survive release.
- November 2: land infrastructure first, then redesign on delivered behavior;
  save net machine resources while keeping active Task switching fast.
- November 4: keep recent unfinished Tasks warm, accepting memory cost for
  instant input; completed Tasks still release on departure through LOO-717.
- November 4: takeover without Fork, preserving last-screen + explicit Take back.
  Shared input was considered and not selected. Navigation never transfers input.
  CLI fork remains independently available on the same Task and checkout.
- November 4: “make sure to prefetch working tasks on load” extends discovery
  beyond saved visits to fresh idle/active/moving Tasks, using the same passive
  reveal and warmth bounds.
- November 4: “ok, proceed” authorizes implementation and review publication
  through interactive demo, not shipping, merge or installation. The accepted
  stack parent is LOO-717/#1811. The stopped November 2 implementation was an
  operator error, not prior work to replay.

Engineering assumptions, not choices attributed to Jack: eight warm unfinished
conversations across repository windows, visible-pane exceptions, selected-first
restore and two concurrent preparations. Memory and usable-input measurements
remain outstanding. Native readiness uses the live relay event separately from
saved frames and program status; configured-provider readiness remains unproven.

No unresolved product choice precedes implementation. The existing plan,
`loo-672-ready-conversations.md`, records the completed entry/restore cut and
integration of 717's owner-evidence refinement; focused verification passed.
Accepted behavior is unchanged.

November 4 concept review: no new question for Jack Heart. The working implementation now keeps missing-history recovery an explicit
**Start conversation** action across ordinary Session refreshes, with current
holder and history evidence supplied by Rust. Fresh repository load also
prefetches working Tasks without saved visits, using the accepted bounds and
Close behavior. Focused headless checks pass. Native input and memory proof remain with demo.

Historical pre-infrastructure proposals (detached quota, old budget, Claude-only
Fork and snapshot construction) are superseded. Their full source is preserved at
`a32c385044:scratch/research/loo-672-pre-infrastructure-plan.md`.

Sync reconciliation — November 4: retain LOO-672’s accepted warm-conversation
policy and review page. Parent LOO-717’s latest design and Rex coordination remain
available through the historical pointers at `b81df60579`; its release/settlement
contract remains integrated. This documentation-only sync changes no runtime policy.
