---
priority: p1
status: open
---
# Import and deploy

Move the website from `studio` into `loopflow/website/` and get it deploying
from this repo. Faithful lift-and-shift — the site is already live with this
content, so this introduces no regression. Truing-up content is item 02.

Full spec in `scratch/website.md`.

## Goal

The site runs here and deploys to fly.io on push to `main`, serving docs from
canonical `docs/`.

## Done when

- `cd website && uv run uvicorn main:app` serves locally, docs rendering from
  canonical `docs/` (materialized into the build context by `dev.py`).
- `website/tests` pass.
- Push to `main` touching `website/**` or `docs/**` deploys `loopflow-website`;
  `https://loopflow.studio/` returns 200; rollback proven on smoke-test fail.
- GitHub Pages publish for `docs/` is disabled.
- `.sesskey` is absent and gitignored.

## Out of band (Jack, not in PR)

- Confirm the website keys are in Doppler `loopflow/prd` (`FLY_API_TOKEN`,
  `WEBSITE_DB_URL`, `RESEND_API_KEY`, `FIGMA_TOKEN`, session key). Deploy pulls
  them via the existing `DOPPLER_TOKEN_PRD` GitHub secret — nothing new in
  GitHub secrets.
- Disable Pages in repo settings once the site's `/docs` is live.
