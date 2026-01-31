# Open Questions

- What is the canonical tenancy model for managed mode: `tenant` + `project`, or a single `org` with projects?
- Do we want event retention defaults (e.g., 30/90 days) baked into config, or leave fully operator-defined?
- Should migration tooling live under `lfd` (Rust) or in a standalone ops command (for CI/DBA use)?
