---
priority: p2
status: open
---
# Content and style alignment

The one-time true-up. The import (item 01, shipped) brought the site over
faithful to studio's frozen state — which is behind the library. This pass
brings content, docs, and code to current reality so the next branch evolves
from a clean base, not a stale import.

The site now lives at `website/`, deploys to fly.io (`loopflow-website`) on push
to `main` via `.github/workflows/website-deploy.yml`, and renders docs from
canonical `docs/` materialized into the build context by `website/dev.py
sync-docs`. `website/docs/` is gitignored — do not commit a second docs source.

## Goal

Content, docs, and vocab match what `lf`/`lfd`/Concerto actually do today; the
code follows repo conventions; the Pages retirement is finished, not half-done.

## Scope (verify against current code before rewriting)

- **Content truth-sync.** `content.yaml` + pages reflect current reality. Known
  stale: "Authentication built in — WorkOS OAuth via Loopflow Studio" — the
  release wave removed the studio control plane in favor of self-hosted
  bearer-token `lfd`. Sweep for other dead claims; align the `vocab` essences
  to the current model. The gate confirmed no hardcoded secrets, but did not
  audit copy for truth — that's this pass.
- **Docs reconciliation.** The deleted `website/docs/` copy was ~63 lines off
  canonical and renamed `lfops.md` → `lfop.md`. Merge any unique content the
  copy had into canonical `docs/`; align `DOCS_NAV` slugs to the canonical set
  (index, getting-started, wave-authoring, waves, lf, lfop, lfd, config,
  troubleshooting).
- **Finish Pages retirement.** Pages disable is a repo-settings op (Jack, out of
  band) once `loopflow.studio/docs` is confirmed live. After that, remove the
  inert `docs/_config.yml` so no Jekyll/Pages residue remains. The website path
  already ignores it, but a clean repo shouldn't carry a dead publish config.
- **Style conformance.** Imports at top, `_` prefix on private functions, drop
  redundant docstrings. Consider splitting `internal_pages.py` (47K) if it's a
  junk drawer rather than cohesive.
- **Repo integration.** Register `website/tests` in `TESTING.md`; make sure CI
  knows about them.

## Done when

- No stale product/auth claims remain in `content.yaml` or pages.
- `docs/` is the sole source; nav matches the canonical file set; `_config.yml`
  is gone once Pages is off.
- Website code passes the repo's Python conventions.
- `TESTING.md` documents how to run website tests.

## Operational prereqs (Jack, outside the PR)

These gate the deploy proving out live; carried over from the shipped import item
because they're still outstanding:

- Confirm the website's keys live in Doppler `loopflow/prd` — `FLY_API_TOKEN`,
  `WEBSITE_DB_URL`, `RESEND_API_KEY`, and `FIGMA_TOKEN`. The deploy pulls them
  via the existing `DOPPLER_TOKEN_PRD` GitHub secret; nothing new goes into
  GitHub secrets.
- Disable GitHub Pages in repo settings once the site's `/docs` is live.
