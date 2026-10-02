# LOO-326 assumptions — 2026-10-02

- Draft policy: Jack's restart-without-Linear outcome covers existing Tasks with
  available, valid cached ownership regardless of observation age. Known
  invalid/removed/terminal/mismatched facts still block. This chooses a scoped
  policy for review; it does not ratify general offline planning.
- Bare restart is the offline acceptance command. New advice remains a Linear
  publication and may fail before replacement. Durable offline advice delivery
  would change the accepted direction owner and is outside this repair.
- “No test waits on the network” means no external service dependency during
  test execution. Local loopback protocol fixtures remain valid; dependency and
  image acquisition happen separately. The known SSH escape is fixed, but the
  suite-wide denial proof remains required.
