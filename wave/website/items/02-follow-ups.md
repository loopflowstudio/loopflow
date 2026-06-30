---
priority: p3
status: open
---
# Follow-ups

Sketches, not commitments. Pick up once the site is moved (done), aligned, and
evolving cleanly here.

- **Broaden the deploy smoke test.** The workflow's rollback is keyed to a single
  `/` 200 check. Browser/accessibility coverage exercises `/docs` and subpages
  locally, but production rollback won't trigger if the homepage stays up while
  `/docs` or a key page breaks. Add a few path checks (`/docs`, `/docs/lfop`,
  `/download`) to the smoke step so a partial outage rolls back.
- **Staging tier.** Port `fly.staging.toml` for a preview environment if we want
  to see changes before production. Deferred from the import deliberately.
- **Release-loop wiring.** Surface website deploy failures into the release
  wave's feedback loop instead of letting them sit in Actions history.
- **Docs freshness checks.** A nightly or per-PR check that flags docs drifting
  from `lf --help` / actual command surface.
