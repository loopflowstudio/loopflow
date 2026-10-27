# LOO-625 direction

- 2026-10-26 Jack Heart DECIDED: `.lf/wave/<name>/config.yaml`, same shape as `.lf/config.yaml`, holding only that Wave's values; GOAL.md stays prose. Supersedes an earlier GOAL.md → WAVE.md frontmatter idea.
  - Layering: `~/.lf/config.yaml` (global) → `.lf/config.yaml` (repo) → `.lf/wave/<name>/config.yaml` (Wave) → Task. One loader, one schema; `lf wave status` reports the layer each value came from.
  - GOAL.md frontmatter (`id`, `linear`, `chat`, `context_budgets`, `agent`, `skill_agents`) moves into the Wave config.yaml; branch reads frontmatter as fallback (shared-file rule) + removal Task (LOO-640).
  - Repo-only keys (`release`, `planning`, `exclude`) in a Wave config.yaml are an error.
- Personal layer for machine/accounts per Wave: local store rows (accepted design, `scratch/loo-625-design.md`).
