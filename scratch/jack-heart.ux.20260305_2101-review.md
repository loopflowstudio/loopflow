# Review: Provider auth detection & eager daemon startup

## Risks to verify

- **Eager start cost on launch** — daemon process starts even if user never opens a repo window. Low cost, but verify no resource leak if app quits before repo window opens.
- **File credential parsing is best-effort** — if Codex or Claude change their auth file format, detection silently returns nil. Acceptable given terminal-assisted flows are the Phase 2 fallback.
- **No cleanup of temp socket files on crash** — `start()` does `removeItem` before bind, so self-heals on next launch. Verify this path works.
