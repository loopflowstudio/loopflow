# Website

Move the Loopflow marketing+docs site out of the private `studio` repo into the
public `loopflow` repo, deploy it from here, and align it to current reality so
future branches evolve it in lockstep with the library.

## Vision

The website lives next to the code it describes. Docs have one source
(`docs/`), the public site serves them, and a change to the library and its
public story land in the same repo — often the same PR. No more private-repo
copy drifting behind what `lf` actually does.

> "it should become a lot easier to keep the public docs and visioning up to
> date with library" — Jack

### Not here

- A redesign of the site. This wave moves and trues-up what exists; new
  visioning is a separate effort once the base is clean.
- Folding the website into the `lfd` self-hosted cron infra. It stays on
  fly.io. Wiring website health into the release feedback loop is a follow-up.
- The root Cargo/uv workspace. The website keeps its own `pyproject.toml` and
  deploys standalone.

## Goals

- `loopflow/website/` holds the site; pushes to `main` deploy it to fly.io
  (`loopflow-website`) with smoke-test + rollback.
- Docs are single-source from `docs/`; the stale `website/docs/` copy is gone
  and GitHub Pages is retired.
- Site content and docs reflect current reality — no dead control-plane
  ("WorkOS OAuth via Loopflow Studio"), vocab matches the current model.
- Code conforms to repo conventions so the next branch starts from a clean base.

## Risks

- **Secret leak into public history.** `.sesskey` is git-tracked in studio;
  it must never land here. Audit content/pages for private URLs or hostnames
  before merge.
- **Deploy regression.** Fly token, build context, and the docs-materialize
  step must all be right or `loopflow.studio` goes down on first push.
- **Drift we don't catch.** The auth copy is the obvious stale claim; there may
  be more. Alignment is only as good as the truth-sync pass.

## Metrics

- Doc sources: 2 → 1 (canonical `docs/` only).
- Doc hosts: 2 (Pages + site) → 1 (site).
- Deploy: push-to-main → live + 200 on `loopflow.studio` with rollback proven.
- Stale product claims in `content.yaml`/pages: → 0.
