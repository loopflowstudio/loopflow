# website: import from studio, deploy from public repo

## What to build

After this, `loopflow/website/` holds the FastHTML marketing+docs site (moved
from the private `studio` repo), it deploys to fly.io as `loopflow-website` on
push to `main`, and it serves docs from the repo's canonical `docs/` — no
duplicate doc copy, GitHub Pages retired.

> "see if we can move all of the loopflow website from the studio repo to here
> ... it should become a lot easier to keep the public docs and visioning up to
> date with library" — Jack

## Decisions (locked)

- **Docs: single source, drop Pages.** `loopflow/docs/` is canonical. Website
  reads from it. Retire the Jekyll GitHub Pages publish so `loopflow.studio/docs`
  is the only doc surface.
- **One PR: move + deploy.** Get it living here and deploying. No staging in
  the first cut unless trivial.
- **Keep `figma.py`.** Dev-time token-puller, secret via env. Ships public.

## The move

Copy `studio/website/` → `loopflow/website/`. Separate repos, so history does
not carry — a plain copy, not `git mv`.

Files that come over:

```
website/
  main.py            # 34K — pages, routing, markdown rendering
  internal_pages.py  # 47K — fonts/colors/design system pages
  content.yaml       # page copy
  db.py              # postgres waitlist + resend notify
  figma.py           # dev design-token exporter
  dev.py             # local dev server
  pyproject.toml     # own deps: fasthtml, resend, psycopg2, uvicorn
  Dockerfile
  fly.toml           # app=loopflow-website, sjc, port 5001
  static/            # logos, screenshots, style.css
  tests/             # accessibility, mobile, e2e (playwright)
```

**Do NOT carry over:**
- `.sesskey` — git-tracked in studio; it's the FastHTML session-signing key.
  Drop it, add to `.gitignore`, regenerate on deploy (env or fly secret).
- `fly.staging.toml` — defer staging unless we decide we want it now.

## Docs: single source

`main.py` has `DOCS_DIR = Path(__file__).parent / "docs"` and a hardcoded
`DOCS_NAV`. The Docker build context is `website/`, so it can't see `../docs`.

Plan:
- `docs/` (repo root) stays canonical. **Delete the imported `website/docs/`
  copy** — never commit the stale duplicate to public history.
- `website/docs/` is gitignored; materialized from `../docs` at build/dev time.
- Deploy workflow copies `docs/*.md` → `website/docs/` before `flyctl deploy`.
- `dev.py` does the same (or symlinks) so local dev sees real docs.
- Reconcile `DOCS_NAV`: canonical file is `lfop.md`; nav currently points at
  `/docs/lfops`. Align slugs to the canonical set
  (index, getting-started, wave-authoring, waves, lf, lfop, lfd, config,
  troubleshooting).

Alternative (rejected for first cut): move Docker build context to repo root
and `COPY website/ docs/`. More invasive to fly config; revisit if the
copy-step feels hacky.

## Deploy

Port `studio/.github/workflows/website-deploy.yml` →
`loopflow/.github/workflows/website-deploy.yml`. It already does the right
things: save current image → `flyctl deploy` → smoke-test `loopflow.studio/`
→ rollback on failure. Changes:

- Add the docs-materialize step before deploy.
- Path filter: trigger on `website/**`, `docs/**`, and the workflow file.

Secrets to add to the **loopflow** repo (manual, Jack — outside this PR):
- `FLY_DEPLOY_TOKEN_WEBSITE_PROD` (fly deploy token)

Fly app secrets stay on fly (unchanged): `WEBSITE_DB_URL`, `RESEND_API_KEY`,
`FIGMA_TOKEN`, session key.

## Public-exposure check

Code is clean — every secret is `os.environ`, nothing hardcoded. Conscious
calls before this lands in public history:
- `content.yaml` / pages name unreleased products (Concerto preview, Symphonia
  interest waitlist). Intended public marketing — fine, but a deliberate yes.
- Scan `internal_pages.py` and `content.yaml` once for anything not meant to
  ship (private URLs, internal hostnames, unreleased roadmap specifics).

## Constraints

- Build context stays `website/` — don't reshape fly/Docker layout in this PR.
- Website keeps its own `pyproject.toml` and deps; it is NOT wired into the
  root Cargo/uv workspace. Standalone app that happens to live in the repo.
- Don't import `website/docs/` into git; canonical `docs/` is the only source.

## Done when

- `cd website && uv run uvicorn main:app` serves the site locally with docs
  rendering from canonical `docs/` (via the materialize step in `dev.py`).
- `website/tests` pass (`uv run pytest` in website/).
- Push to `main` touching `website/**` triggers the workflow, deploys to
  `loopflow-website`, and `https://loopflow.studio/` returns 200.
- GitHub Pages publish for `docs/` is disabled; `loopflow.studio/docs` is the
  live doc surface.
- `.sesskey` is absent from the repo and gitignored.

## Follow-ups (not this PR)

- Staging environment (`fly.staging.toml`) if we want a preview tier.
- Wire website freshness into the release wave's cron/feedback loop.
