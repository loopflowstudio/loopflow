# Stability

Loopflow is always shippable and always up: releases run themselves and
prod — main, the hosts, the daemons, the live waves — stays healthy
indefinitely, with every failure surfacing as focused work instead of
silent drift.

## KRs

- Nightly verification and weekly release complete for consecutive cycles
  with no manual repair (Linear 6092ca8).
- Main stays green; the self-hosted lfd host and the cron host stay up;
  hosts stay fresh (the sync-skills --global failure class — found
  2026-07-08 only by log audit — never recurs unnoticed).
- Billing and agent spend stay bounded, visible, and unsurprising.
