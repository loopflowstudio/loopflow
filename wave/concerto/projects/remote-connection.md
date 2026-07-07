# Remote connection

Shipped and maintained. Reach a native remote `lfd` over HTTPS via Tailscale.
Watch for regressions; don't expand speculatively.

## KRs

- Concerto reaches a native remote `lfd` over HTTPS via Tailscale (TLS
  terminates outside the daemon).
- Bearer-token rotation takes effect without re-pasting into Settings.

## Notes

Deliberately not built: multi-profile config, a live-tailnet CI round-trip,
TLS inside `lfd` (rejected, not a gap). Revisit only when a second host exists.
