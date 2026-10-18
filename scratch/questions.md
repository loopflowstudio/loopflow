# LOO-416 assumptions and review choices — 2026-10-16

Jack Heart accepted the architectural direction: shared lfdb layout storage keyed by Work;
ordinary shells reconnect to surviving per-shell PTY owners. Jack subsequently
requested evaluation of prefetching for every ensured Session, including nested
CLI creation. Warming policy is under investigation; discovery alone does not
resume ended execution. Detailed restoration mechanisms remain to be proved.

Consequential choices retained for design review:

- Exact terminal restoration exceeds LOO-415's implemented vt100 text snapshot
  and 2,000-row scrollback. Full buffer/mode restoration and renderer-owned state
  remain unresolved within that owner boundary; native history cannot replace them.
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
LOO-497 native integration. LOO-415 PR #1581 is now integrated; use its attach-only
API and preserve its honest terminal-state limits. LOO-497's shared pane/draft
contract has a local implementation awaiting supported published handoff.

October 16 implementation resolves the operator's reopening finding:
`openRepository` presents retained content before lookup. Its proven-locator index
includes worktree paths and restored scenes; it grants presentation only. Fresh
validation still owns Task-link delivery and opening receipts. Headless tests cover
success, failure and cancellation without changing retained panes/drafts. An active
Task-link opening takes precedence over restored selection when the model binds.

# LOO-415 assumptions

October 17, 2026: use a fixed five-second acknowledgement deadline after connecting
to the local owner socket. This bounds the identified silent-owner wait without a
new configuration knob; it does not bound SSH startup or an established attachment.
Cancellation uses the existing CLI signal handler and closes only the viewer.

October 17: operator steer 1482 authorizes uncaptured admission within LOO-416.
Task creation and explicit Wave admission now commit primary Session identities;
ensure prepares location, and explicit execution reserves the first input. No
provider warming policy was selected. Reconnect remains attach-only, with separate
explicit recovery. Full restoration/readiness acceptance remains in the plan.
Shared reads and naming recovered with released lf 0.14.4 pinned through LF_BIN;
installation routing, not authentication, caused the initial read gap.

October 17 sync resolution: retain the six conflicted scratch summaries because
LOO-416 still uses their accepted direction and open questions. Parent scratch
cleanup does not retire this branch’s active plan. Combine non-displacing viewer
attachment with the parent’s EOF and owner-exit fixes; retain both test sets.

October 17 bounded-query assumption: finite Desktop CLI requests have a 30-second
monotonic deadline and a 64 MiB limit per output pipe. Cancellation/timeout kills
only the exact query child; accepted execution owners survive. A timed-out mutation
reports an unknown result requiring refresh, never automatic retry. Long-lived
work and observation keep their existing distinct launch paths.
