# Open choices — LOO-443

2026-10-09 implementation choice: fresh opaque attachment tokens fence each
claim and release, including A → B → A. This resolves the repeated-ID ambiguity
without another process identity or lifecycle. The single-table AgentProcess cut
must move the token and attached LfProcess onto that record; provider generation
still exists in this internal slice. No approval beyond Jack Heart's original
Task directive is inferred.

2026-10-09: Source OpenCode launches are one-server/one-Session. Historical
shared PID/birth rows must remain recoverable and non-signallable until exact
ownership is resolved; source inventory is not an audit of configured data.

2026-10-09 implementation choice: native request attribution freezes before
sending rather than on a delayed Started observation. A later bind affects later
requests, not already-submitted work. Per-request correlation—not AgentProcess's
original capture—retains this snapshot across live takeover and multiple turns.
Crash-lost correlation remains unknown; this introduces no recovery replay or
new execution owner.

2026-10-09 record-cut finding: Claude interrupt/respawn reuses a cloned attachment
inside one capture. Each spawn needs a fresh AgentProcess, with explicit handoff
of the new attachment to the existing capture owner and harness. A mutable shared
snapshot must not revive stale operations. The current identity writer refuses
PID replacement; that protects history but is not accepted interrupt/resume
behavior. Further launch work stops at this ownership correction. No new product
decision or additional lifecycle owner is proposed.
