# LOO-416 assumptions and review choices — 2026-10-16

Jack Heart accepted the architectural direction: shared lfdb layout storage keyed by Work;
ordinary shells reconnect to surviving per-shell PTY owners. Jack subsequently
requested evaluation of prefetching for every ensured Session, including nested
CLI creation. Warming policy is under investigation; discovery alone does not
resume ended execution. Detailed restoration mechanisms remain to be proved.

Consequential choices retained for design review:

- Exact terminal restoration needs a mechanism beyond transparent bytes and repaint.
  Evaluate a library-backed headless state model within LOO-415's owner, preserving
  its transport boundary. Do not reduce scrollback acceptance to native history.
  Client-only selection/IME composition needs its own recovery limits.
- D9 mentions reopening on another machine. The app-side store supports one
  viewer switching execution Machines. Layout replication between separate
  Desktop installations still needs a persistence/synchronization owner.
- LOO-497 subsequently records Jack Heart selecting first-wins presentation
  for overlapping query matches; this is settled direction, with integration
  pending. Shared storage/arrangement foundations are authorized before prototype
  review; production query grammar/UX remain with LOO-497/489.

The subsequent conversation approved ownership direction, not answers to these
remaining questions. These choices limit acceptance;
the draft preserves them rather than silently reducing the requested outcome.

Implementation assumption: keep application-lifetime retention independent of the
pending LOO-497 storage contract. LOO-415 has no delivered attach-only API;
transport-dependent implementation remains blocked, without launching that Task.

October 16 implementation resolves the operator's reopening finding:
`openRepository` presents retained content before lookup. Its proven-locator index
includes worktree paths and restored scenes; it grants presentation only. Fresh
validation still owns Task-link delivery and opening receipts. Headless tests cover
success, failure and cancellation without changing retained panes/drafts. An active
Task-link opening takes precedence over restored selection when the model binds.
