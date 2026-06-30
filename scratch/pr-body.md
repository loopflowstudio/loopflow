## Try it!

```bash
cd website
uv run python dev.py test
uv run python dev.py serve
```

Open `http://127.0.0.1:5001/` and check `/docs`, `/docs/lfop`, `/download`, and `/concerto`.

For the deploy image shape:

```bash
cd ..
docker build -t loopflow-website-gate website
docker run --rm -p 5017:5001 loopflow-website-gate
curl -I http://127.0.0.1:5017/
curl -I http://127.0.0.1:5017/docs
```

Gate results: website tests passed with 61 passed, 3 skipped; `uv run ruff check website` passed; Docker build and container smoke passed for `/`, `/docs`, `/docs/lfop`, and `/install.sh`.

## Intent

Move the public Loopflow marketing and docs site from the private studio repo into this repo so the website, docs, and library can evolve together. The site deploys to Fly from pushes to `main`, while root `docs/` remains the only committed docs source.

## Assumptions

- Doppler `loopflow/prd` provides `FLY_API_TOKEN`, `WEBSITE_DB_URL`, `RESEND_API_KEY`, `FIGMA_TOKEN`, and the website session key.
- GitHub Pages will be disabled out of band after `loopflow.studio/docs` is live.
- First cut is a faithful move; broader copy/style alignment stays in the next website wave item.

## Key decisions

- Keep the website standalone under `website/` with its own dependencies and Fly config.
- Keep Fly's Docker build context at `website/`; materialize canonical root docs into `website/docs/` before local test/dev and before deploy.
- Keep `figma.py` as a dev-time token exporter, with all sensitive values read from environment variables.
- Add `website/.dockerignore` so local deploy images cannot package `.sesskey`, virtualenvs, caches, or tests.

## Not included

- No staging deployment path.
- No content truth-sync beyond the imported baseline.
- No live production deploy from this gate pass.
- No repo-settings change to disable GitHub Pages.
