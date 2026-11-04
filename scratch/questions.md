# LOO-672 — Accepted decisions

Jack Heart selected:

- November 4 re-demo: “but the conversation should be auto stated and opened”
  and “thats teh whole pointof this task”. Working Tasks automatically establish
  and open their primary conversation on load/selection, including absent Session
  and missing native history. No Start/Open click. This supersedes the earlier
  explicit-Start rule; it was the wrong constraint for this experience.

- October 28, 2026: return at the bottom; exact scroll position need not survive release.
- November 2: land infrastructure first, then redesign on delivered behavior;
  save net machine resources while keeping active Task switching fast.
- November 4: keep recent unfinished Tasks warm, accepting memory cost for
  instant input; completed Tasks still release on departure through LOO-717.
- November 4: takeover without Fork, preserving last-screen + explicit Take back.
  Shared input was considered and not selected. Navigation never transfers input.
  CLI fork remains independently available on the same Task and checkout.
- November 4: “make sure to prefetch working tasks on load” extends discovery
  beyond saved visits to fresh idle/active/moving Tasks, using bounded preparation and warmth with non-replacing ownership.
- November 4: “ok, proceed” authorizes implementation and review publication
  through interactive demo, not shipping, merge or installation. The accepted
  stack parent is LOO-717/#1811. The stopped November 2 implementation was an
  operator error, not prior work to replay.

Engineering assumptions, not choices attributed to Jack: eight warm unfinished
conversations across repository windows, visible-pane exceptions, selected-first
restore and two concurrent preparations. Memory and usable-input measurements
remain outstanding. Native readiness uses the live relay event separately from
saved frames and program status; configured-provider readiness remains unproven.

Historical pre-infrastructure proposals (detached quota, old budget, Claude-only
Fork and snapshot construction) are superseded. Their full source is preserved at
`a32c385044:scratch/research/loo-672-pre-infrastructure-plan.md`.

Historical correction before the latest re-demo clarification (superseded where
it requires a Start click): The existing resume explanation
supplies the absent-Session action; Start now uses the shared non-replacing opening
path through execution and transport claim. Only explicit Take back authorizes
replacement, including when ownership changes after preparation.
The candidate uses disposable planning/checkout data and prepared native-provider
metadata. No real provider has launched in this headless pass; native input,
drafts and footprint remain demo evidence, not assumptions of success.
