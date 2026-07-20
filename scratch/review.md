# Simulated operational review

## Findings fixed

- A separate Home resident created a second boot authority and could make
  `lfd` startup a no-op. `lfd` now owns an in-process `WaveHost` and one
  reconciliation loop.
- One broken Wave could stop later siblings from starting. `WaveHost` now
  attempts every requested Wave and returns the collected failures afterward.
- The WaveHost reconciliation task could race graceful shutdown. Shutdown now
  aborts and joins that task before stopping listener children.
- Security documentation called `LF_LFD_AUTH_TOKEN` request authentication even
  though the runtime only uses its presence as permission to bind off loopback.
  The docs and warning now state that distinction and require a real network
  boundary for exposed health/status routes.

## Deliberate bounds

- `owner` and `home` are automatic-start policy, not authorization.
- HomeId is the stable preferred value. Loopback, hostnames, local interface
  addresses, and the current SSH destination are accepted when Loopflow can
  prove them. A NAT-only public address is not guessed from inside the guest.
- The WaveHost reconciles every 30 seconds, so newly registered eligible Waves
  and exited eligible listeners are picked up without restarting `lfd`.
