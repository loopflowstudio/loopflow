# Open questions

- Does release promotion stop and restart the current Task process, or allow
  adjacent releases to coexist under an explicit ownership boundary?

# Decisions

- On 2026-09-02, Project and Task controller liveness and signaling were bound
  to one Work-linked, birth-validated OS Exec owner. Provider Invocation,
  provider session, and Run state remain advisory: they cannot authorize a
  signal or veto recovery after positive OS absence. A live birth with missing
  or contradictory ownership fails closed, as do multiple validated owners;
  tmux remains transport rather than authority.

# Learnings

- On 2026-09-02, Project and Task controller launch stopped treating detached
  tmux creation as startup proof. An attempt-scoped receipt now distinguishes a
  real body with a provider Run and exact live Exec owner from a parked human
  node or a failed child; the launcher records a resumable Work failure when
  the child cannot do so itself.
- Installed controller executables are content-addressed as
  `lf-<64-hex-digest>`. Process inspection must recognize that name alongside
  the development `lf` binary or it will hide valid ownership evidence.
