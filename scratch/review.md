# Simulated operational review

## Findings fixed

- An already-live Home resident made `lfd` startup a no-op. `lfd` now sends the
  freshly selected Wave ids after `ensure`, so daemon restart always reconciles
  the current eligible set.
- One broken Wave could stop later siblings from starting. The resident now
  attempts every requested Wave and returns the collected failures afterward.
- The resident's automatic-start task could race graceful shutdown. Shutdown
  now aborts and joins that task before stopping listener children.
- Security documentation called `LF_LFD_AUTH_TOKEN` request authentication even
  though the runtime only uses its presence as permission to bind off loopback.
  The docs and warning now state that distinction and require a real network
  boundary for exposed health/status routes.

## Deliberate bounds

- `owner` and `home` are automatic-start policy, not authorization.
- HomeId is the stable preferred value. Loopback, hostnames, local interface
  addresses, and the current SSH destination are accepted when Loopflow can
  prove them. A NAT-only public address is not guessed from inside the guest.
- The resident reconciles at resident or `lfd` startup. Registering a new Wave
  while both are already live still requires explicit `lf start <wave>`.
