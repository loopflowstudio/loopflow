# website wave memory

Steers the Loopflow marketing+docs site into the public repo: one docs source, deployed from here, content aligned to current reality so the site evolves in lockstep with the library.

## Shipped

- **Site import** — the site now lives at `website/`, deploys to fly.io (`loopflow-website`) on push to `main` via `.github/workflows/website-deploy.yml`, and renders docs from canonical `docs/` materialized by `website/dev.py sync-docs`. `website/docs/` is gitignored.

## Model (design invariants)

- The website lives next to the code it describes; a library change and its public story land in the same repo, often the same PR.
- Docs have one source (`docs/`); the public site serves them, GitHub Pages is retired.
- The site stays on fly.io with its own `pyproject.toml` — not folded into `lfd` cron infra or the root workspace.
- `.sesskey` and any studio secrets must never land in public history.

## Next

- **Content and style alignment** (`items/01`) — truth-sync `content.yaml`/pages (kill the stale "WorkOS OAuth via Loopflow Studio" auth claim), reconcile docs + `DOCS_NAV` to canonical set, finish Pages retirement (`docs/_config.yml` removal), style conformance, register `website/tests` in `TESTING.md`.
- **Follow-ups** (`items/02`) — broaden deploy smoke test to `/docs`, `/docs/lfop`, `/download`; port `fly.staging.toml` staging tier; wire deploy failures into the release loop; docs-freshness checks against `lf --help`.
