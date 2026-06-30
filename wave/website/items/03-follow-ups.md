---
priority: p3
status: open
---
# Follow-ups

Sketches, not commitments. Pick up once the site is moved, aligned, and
evolving cleanly here.

- **Staging tier.** Port `fly.staging.toml` for a preview environment if we
  want to see changes before production.
- **Release-loop wiring.** Surface website deploy failures into the release
  wave's feedback loop instead of letting them sit in Actions history.
- **Docs freshness checks.** A nightly or per-PR check that flags docs drifting
  from `lf --help` / actual command surface.
