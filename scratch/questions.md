# LOO-534 open questions

- **Sidebar regression target (2026-10-20).** The brief asks to cover the sidebar
  fix from 2f0fac21a1 (#1443: settled Tasks leave the sidebar). Jack Heart's
  October 17 LOO-489 Console decision superseded it: "Completion does not
  automatically remove retained work". Assumption: cover the current contract
  (completed/abandoned retained, paused hidden). Jack decides whether settled
  Tasks should leave the working sidebar again.
- **Completed history in the live Wave inventory (revised 2026-10-20, publish).**
  #1594 (LOO-489) asserts the live inventory shows the full current plan with no
  `TaskHistoryControls`; completed history lives behind Project history. Decision:
  the inventory drops settled Tasks and adds no control. Jack decides whether the
  live inventory should gain a Completed range control.
- **"Canceled upstream" without Linear.** The recipe writes a planning-canceled
  disposition through the observation writer on a local-only Task. Real
  instances arrive from Linear. Assumption: this is faithful enough for view
  coverage, and a Linear-linked lifecycle stays out of scope.
- **Upstream-canceled Task with started work (2026-10-20, implement).** The plan
  expected it out of the current inventory. It has started local work, so
  `unresolved_execution` keeps it current (condition `waiting · ready`), matching
  "uncertain evidence never auto-closes work". Assumption: that is correct; the
  recipe and tests assert it stays current and never enters Completed. Jack
  decides whether an upstream cancel should hide started work.
