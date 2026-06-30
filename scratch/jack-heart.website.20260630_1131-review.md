# Website Import Gate Review

## What was implemented

The FastHTML website now lives under `website/` in this repo, with marketing pages, docs rendering, static assets, local dev/test helpers, browser/accessibility tests, Fly config, and a production deploy workflow for `loopflow-website`.

Docs remain canonical in root `docs/`. Local development and tests materialize them into `website/docs/`; the deploy workflow does the same before building the Fly image. `website/docs/` stays ignored so the public repo does not gain a second docs source.

## Key choices

- Kept the website as a standalone Python app with its own `pyproject.toml` and `uv.lock`, matching the design constraint to avoid folding it into the root workspace.
- Kept the Docker build context at `website/`, and materialized docs into that context before deploy instead of restructuring Fly to build from the repo root.
- Added `website/.dockerignore` during gate so ignored local files like `.sesskey`, `.venv`, caches, and tests cannot be packaged into local Fly/Docker images.
- Declared `pyyaml` as a direct website dependency because `main.py` imports `yaml`; the app no longer relies on a transitive dependency.
- Updated root package metadata to point Homepage/Documentation at `loopflow.studio` rather than the retired GitHub Pages URL.

## How it fits together

`website/main.py` serves the public pages and renders Markdown docs from `website/docs/`, falling back to root `docs/` for local runs from a full checkout. `website/dev.py sync-docs` copies canonical docs into the website build context and rewrites the install URL for the hosted site. `.github/workflows/website-deploy.yml` fetches Doppler secrets, materializes docs, stages runtime secrets into Fly, deploys, smoke-tests `https://loopflow.studio/`, and rolls back to the prior image if the smoke test fails.

## Risks and bottlenecks

- The production deploy still depends on Doppler `loopflow/prd` containing `FLY_API_TOKEN`, `WEBSITE_DB_URL`, `RESEND_API_KEY`, and `FIGMA_TOKEN`.
- FastHTML creates a runtime `.sesskey` when the app starts, but the key is not packaged into the Docker image. If the site starts relying on server-side sessions, a stable session secret should be wired explicitly.
- GitHub Pages retirement is partly a repo-settings operation; the branch removes the website/docs duplication path but cannot disable Pages from local code.
- The deploy workflow smoke test only checks `/`. Browser coverage exercises docs and subpages locally, but production rollback is keyed to the homepage status.
- `docs/_config.yml` remains for now. It is inert for the website path, but should be removed once Pages is confirmed disabled if the repo wants no Pages residue.

## What's not included

- No staging Fly app.
- No content truth-sync beyond the lift-and-shift scope; stale product claims are item 02 in `wave/website`.
- No live production deploy was run from this gate pass.
- No GitHub Pages settings change was made locally.

## Validation

- `cd website && uv run python dev.py test` -> 61 passed, 3 skipped.
- `uv run ruff check website` -> passed.
- `docker build -t loopflow-website-gate website` -> passed.
- Container smoke from `loopflow-website-gate`:
  - `/` -> 200
  - `/docs` -> 200
  - `/docs/lfop` -> 200
  - `/install.sh` -> 302
- Image packaging check before app startup confirmed `/app/.sesskey`, `/app/.venv`, and `/app/tests` are absent. Runtime app startup creates `/app/.sesskey`.
