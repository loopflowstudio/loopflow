# Open questions / assumptions

## pm-linear-roadmap-access

- **Assumption: this is a stale-binary problem, not a code bug.** Verified by
  building HEAD (0.10.0) and running `lf op pm show --wave architecture` — it
  reaches Linear and lists the roadmap. The deployed `command lf` is 0.9.12
  (Asana-only app-bundle binary shadowing PATH). No `resolve_provider` change
  made.
- **Did not run the redeploy in-session.** `install.py local --use` can restart
  the `lfd` launchd service, which would disrupt the live wave server that
  dispatched this work. Left the deploy for a step run outside the wave loop (or
  `install.py refresh` once the change is on main). The fix is operational, not
  a PR diff — flagged for whoever lands it.
- **Open: should lfd guard against exec'ing a stale `lf`?** Sketched in the
  design doc as a design-gated follow-up, not part of this unblock.
