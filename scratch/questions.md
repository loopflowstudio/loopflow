# Open questions / assumptions

## pm-linear-roadmap-access

- **Assumption: this is a stale-binary problem, not a code bug.** Verified by
  building HEAD (0.10.0) and running `lf op pm show --wave architecture` — it
  reaches Linear and lists the roadmap. The deployed `command lf` was 0.9.12
  (Asana-only app-bundle binary shadowing PATH). No `resolve_provider` change was
  needed.
- **Resolved in-session.** `scripts/install.py local --use` now promotes the app
  into the active `Loopflow.app` bundle when bare `lf` resolves there, and skips
  the obsolete `uv tool install` step for the Python wheel. Reran with
  `LOOPFLOW_CODESIGN_IDENTITY=-` to avoid a headless keychain prompt. Bare
  `lf --version` is now 0.10.0 and `lf op pm show --wave architecture` lists the
  Linear roadmap.
- **Open: should lfd guard against exec'ing a stale `lf`?** Sketched in the
  design doc as a design-gated follow-up, not part of this unblock.
