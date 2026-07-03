---
primary_flow: ship-wave
pm:
  provider: asana
  asana_project: ''
---

The website lives next to the code it describes: docs single-sourced from
`docs/`, deployed from this repo to fly.io, content true to what `lf` actually
does — so the library and its public story evolve in lockstep.

**Metrics to improve**
- Doc sources: 2 → 1 (canonical `docs/` only)
- Doc hosts: 2 (Pages + site) → 1 (site)
- Deploy: push-to-main → live + 200 on loopflow.studio, rollback proven
- Stale product claims in `content.yaml`/pages → 0

**Milestones**
- Truth-sync `content.yaml`/pages; kill the stale WorkOS-auth claim
- Reconcile docs + `DOCS_NAV` to the canonical set; finish Pages retirement
- Register `website/tests` in `TESTING.md`
