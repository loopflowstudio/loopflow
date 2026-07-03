# mobile wave memory

Archived wave: loopflow ships no mobile surface — the phone story is the vendors' own apps (Claude, Codex), and mobile returns only as a thin reader on the desktop lfd HTTP API.

## Shipped

- **Archived 2026-06-19** — reversed the read-only iOS charter. Built the connection path (pairing, remote-lfd auth), then retired it: pairing UI under `Platform/iOS/`, `lf op pair` and pairing tokens, remote-lfd-for-phone infra, and the `remote-lfd-connection` / `see-your-waves` / `see-wave-tasks` roadmap items (all in git history). See `release/unreleased/DECISIONS.md` (2026-06-19, "Loopflow is the layer above").

## Model

- Mobile happens through the vendors' own apps, not a loopflow iOS app.
- If it comes back, it is a thin reader on the same lfd HTTP API the desktop uses — not a session host.
- It does not come back before the desktop layer-above story is solid.

## Next

- (none — wave archived; revisit only after the desktop layer-above story is solid)
