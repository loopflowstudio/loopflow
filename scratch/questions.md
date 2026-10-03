# LOO-326 decisions and bounds — 2026-10-02

- Approved by Jack Heart on 2026-10-02: restart-without-Linear covers existing Tasks with
  available, valid cached ownership regardless of observation age. Known
  invalid/removed/terminal/mismatched facts still block. This is a scoped
  existing-work policy; it does not ratify general offline planning.
- Bare restart is the offline acceptance command. New advice remains a Linear
  publication and may fail before replacement. Durable offline advice delivery
  would change the accepted direction owner and is outside this repair.
- “No test waits on the network” means no external service dependency during
  test execution. Local loopback protocol fixtures remain valid; dependency and
  image acquisition happen separately. The known SSH escape is fixed, but the
  suite-wide denial proof remains required.

- Jack Heart approved the design and waived a personal demo on 2026-10-02.
  Autonomous implementation and delivery retain automated gate/CI acceptance.
  No unresolved product decision remains from this review.
