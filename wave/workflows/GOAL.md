---
primary_flow: build
pm:
  provider: asana
  asana_project: '1214270115593839'
---

The engine: scheduling, providers, flow execution, mutation, and the governance surfaces that expose all of it coherently. Drive toward waves that ship overnight while PM and the Concerto surfaces stay a truthful mirror of the work.

**Metrics to improve**
- Loop-mode waves ingest from PM and ship PRs overnight without babysitting
- PM state mirrors wave and PR reality
- Governance surfaces read from one engine-backed model

**Milestones**
- Release infra: shared cadence, local `lf`/`lfd` refresh, self-hosted cron host, budget guardrails
- Daily garden cycle produces reviewable mutation PRs on cron
- Continuous build loop ingests, ships, and reports lifecycle unattended
- PM round-trip: dependency + lifecycle sync and one-command reset tooling
