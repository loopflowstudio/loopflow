# website: import from studio, deploy from public repo

Item 01 of `wave/website` — the faithful lift-and-shift of the FastHTML
marketing+docs site from the private `studio` repo into `loopflow/website/`,
deploying to fly.io (`loopflow-website`) on push to `main`. Shipped on this
branch. Content/style truing-up and Pages cleanup carry forward in
`wave/website/items/01-content-and-style-alignment.md`.

## Validate this branch

```bash
cd website
uv run python dev.py test          # 61 passed, 3 skipped at gate
uv run ruff check .                # passes
uv run python dev.py serve         # http://127.0.0.1:5001/
```

Open the local site and check `/`, `/docs`, `/docs/lfop`, `/download`,
`/concerto`. Docs render from canonical root `docs/`, materialized into the
build context by `dev.py sync-docs` — there is no committed `website/docs/`.

Deploy image shape:

```bash
docker build -t loopflow-website-gate website
docker run --rm -p 5017:5001 loopflow-website-gate
curl -I http://127.0.0.1:5017/        # 200
curl -I http://127.0.0.1:5017/docs    # 200
```

Gate also confirmed the packaged image excludes `.sesskey`, `.venv`, and
`tests/` (via `website/.dockerignore`), and that `.sesskey` is untracked and
gitignored.

## What's not proven here

- No live production deploy was run from the gate — the workflow exists and
  smoke-tests `https://loopflow.studio/` with rollback, but first real deploy
  happens on merge.
- Deploy depends on Doppler `loopflow/prd` holding `FLY_API_TOKEN`,
  `WEBSITE_DB_URL`, `RESEND_API_KEY`, `FIGMA_TOKEN`, and the session key, pulled
  via the existing `DOPPLER_TOKEN_PRD` GitHub secret.
- GitHub Pages disable is a repo-settings op, done out of band once
  `loopflow.studio/docs` is live.
