---
priority: p2
status: open
---
# Content and style alignment

The one-time true-up. After the move, the site is faithful to studio's frozen
state — which is behind the library. This pass brings content, docs, and code
to current reality so the next branch evolves from a clean base, not a stale
import.

## Goal

Content, docs, and vocab match what `lf`/`lfd`/Concerto actually do today; the
code follows repo conventions.

## Scope (verify against current code before rewriting)

- **Content truth-sync.** `content.yaml` + pages reflect current reality. Known
  stale: "Authentication built in — WorkOS OAuth via Loopflow Studio" — the
  release wave removed the studio control plane in favor of self-hosted
  bearer-token `lfd`. Sweep for other dead claims; align the `vocab` essences
  to the current model.
- **Docs reconciliation.** The deleted `website/docs/` copy was ~63 lines off
  canonical and renamed `lfops.md` → `lfop.md`. Merge any unique content the
  copy had into canonical `docs/`; align `DOCS_NAV` slugs to the canonical set.
- **Style conformance.** Imports at top, `_` prefix on private functions, drop
  redundant docstrings. Consider splitting `internal_pages.py` (47K) if it's a
  junk drawer rather than cohesive.
- **Repo integration.** Register `website/tests` in `TESTING.md`; make sure CI
  knows about them.

## Done when

- No stale product/auth claims remain in `content.yaml` or pages.
- `docs/` is the sole source; nav matches the canonical file set.
- Website code passes the repo's Python conventions.
- `TESTING.md` documents how to run website tests.
