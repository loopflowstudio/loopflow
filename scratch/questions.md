# LOO-447 — unresolved transport cut (2026-10-10)

The [plan](stop-and-take-over-claude.md) retains the counterexamples and full
acceptance: Claude needs launcher-independent transport and pending correlation;
OpenCode needs reconnect plus fenced stop/abort/drop. A named lifeline alone is
insufficient. Neither finding proves takeover impossible or authorizes a refusal.
The common-close slice is not independently shippable.

The named-lifeline draft also needs an attacher-first release protocol: immediate
superseded-holder release can kill the provider when the new attacher dies;
unbounded retention preserves the existing leak. Transport and lifetime design
remain implementation work, not a request to waive takeover.
