# Door auth

Harden the wave-server exec door: a leaked token must not become arbitrary
unsandboxed execution.

## KRs

- `/v0/exec` verb allowlist reviewed against the flowloop-era commands; deny
  list covers loop starters (`task`, `flow`, `skill` without `--dispatch`)
  (Linear dc4391a2).
- The resident token upgrade path (gatekeeper-issued credential) is designed
  when a human or remote steward can hold the seat.
