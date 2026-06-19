# Mobile — archived 2026-06-19

**Archived.** Loopflow does not ship a mobile surface. Mobile happens through the
vendors' own apps (Claude, Codex), not a loopflow iOS app.

See `release/unreleased/DECISIONS.md` (2026-06-19, "Loopflow is the layer above").

## Why archived

The original charter was a read-only iOS surface — connect to a remote lfd, see
your waves, read the roadmap. We built the connection path (pairing, remote-lfd
auth) and then reversed it. Reimplementing a mobile client doesn't compound when
the vendors ship better mobile apps than we will. The phone story is: use the
Claude / Codex apps.

## What this retires

- iOS target and `Platform/iOS/` (handled in the teardown branch)
- `lf op pair`, pairing tokens, remote-lfd-for-phone connection infra
- The roadmap items: `remote-lfd-connection`, `see-your-waves`, `see-wave-tasks`
  (in git history)

## If mobile comes back

It comes back as a thin reader on the same lfd HTTP API the desktop uses — not as
a session host, and not before the desktop layer-above story is solid.
