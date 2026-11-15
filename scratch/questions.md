# Working assumptions

- Scope now covers Desktop and mobile interaction state throughout the app;
  launch, completion, cached/recent Task selection and Wave disclosure are
  representative acceptance journeys. The earlier
  Desktop-only boundary is superseded by comments b38d7ddf/e008238d/30bf26ae.
- Headless engineering choice: use authoritative cache plus ordered operation
  overlays and transactional outbox for the next implementation; no requirement
  calls for a full mutable offline replica. This is not a recorded product
  preference. Validate membership coverage and GRDB/toolchains during implementation;
  GRDB is not adopted yet. Protocol revision 6 now adds stamped subscriptions;
  transactional cache/outbox and membership migration remain unfinished.
- Completion navigation: Jack Heart replied “1b for the question (or show noting)”
  to the numbered choice in `scratch/concept-review.html`. Remove the row and
  select the next Task immediately; an empty state is allowed when none remains.
  This supersedes the draft keep-conversation-open assumption. Failure or refresh
  never steals a later deliberate selection.
- Proposed 100 ms P95 local reaction target, at least 20 native samples per
  journey/platform. Native reaction, input readiness and backend timings remain
  unmeasured; no SQL diagnosis is asserted. Implementation now owns benchmark
  collection too. Both hosts failed load preflight; the mini also runs macOS 15.5
  against Desktop's macOS 26 target. Current debug 10k constructor samples are
  diagnostic only; a valid native baseline remains open.

- Admission identity: headless engineering choice is immutable submitted content
  plus deferred canonical creation identity. Released rows bind the first verified
  envelope through recoverable candidate validation in the same receipt owner;
  candidate conflict never replaces the existing creation outcome. This remains
  proposed implementation, not a recorded product preference.
