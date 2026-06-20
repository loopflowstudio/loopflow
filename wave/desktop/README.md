# Desktop

Concerto for macOS. The layer above the vendors' sessions.

## Vision

Concerto frames work; the vendors run it. The conductor opens the app, launches a
session, and that session is the vendor's own — a TUI (Claude Code, Codex,
opencode) inside an embedded terminal pane, or a bounce out to the vendor's
standalone app. Concerto keeps embedded sessions alive across restarts and shows
wave state around them. It is not a chat client and does not render assistant turns
itself.

See `release/unreleased/DECISIONS.md` (2026-06-19, "Loopflow is the layer above").

### Not here

- A native chat UI — we do not reimplement the vendors' chat. Dropped.
- Replacing the CLI — the CLI stays the source of truth; Concerto composes the
  work around it
- Governance dashboards, calibration, portfolio, beat programming — those belong
  to `workflows`
- The launch mechanism itself — `lf`-first vendor-session launch lives in
  `workflows` (`vendor-session-launch`); desktop consumes it

## Tasks

1. **`embedded-terminal-build-driver`** (p1) — terminal launch, reattach,
   multi-agent dispatch, terminal tabs, and window polish compose into one daily
   surface. The pane hosts the vendor's TUI; Concerto frames it, doesn't render
   chat.

## Risks

- Embedded terminal parity has a ceiling — for some sessions the vendor's
  standalone app will win; make the bounce-out a first-class action, not a
  fallback
- Build-driver polish can sprawl; anchor the finish line to daily use
- The "frame, don't render" line will get pressure to creep back into a chat
  client; hold it
