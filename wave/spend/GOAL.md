---
pm:
  linear_initiative: 4b848b46-1fb9-446f-a06e-f0038861cb23
id: e45d2618-1b7c-4686-91ea-342350763bf3
---
## Objective

Jack Heart can understand spending across repositories and Waves, see which work
and resources drive LLM and infrastructure costs, and delegate financial analysis
through narrowly scoped read access. Reports connect cost with token usage,
performance and work outcomes, with basic bank financials as the overall spending
baseline.

## Responsibility

Spend owns the combined cost picture: provider billing and usage ingestion,
repository and Wave attribution, explicit shared-cost allocations, reporting, and
credential operations for its integrations. Its scope crosses repositories even
though this Wave is authored in the Loopflow repository.

Computing dependencies connect the access and spending views: what software and
agents rely on, which repositories and Waves consume it, who can access it, and
what it costs. Dependencies, provider accounts, Doppler credentials and billing
sources are not one-to-one. Keep unknown costs visible; expose richer usage,
cost breakdowns and granular credential scopes for scalable cloud and AI services
where supported. Ordinary subscriptions can have simpler access and charge data.

Reuse Loopflow's recorded Session, Task, repository and Wave identities and usage
history. Intelligence owns the underlying usage/performance evidence; Infrastructure
owns execution and general authentication mechanisms; Spend owns the financial
interpretation and restricted-access requirements. Product owns shared Loopflow UX.

Show direct attribution, allocations, estimates, billed charges and bank payments
as distinct evidence. Preserve historical attribution; later Task moves do not
silently rewrite prior spending. Keep shared and unassigned costs visible. Every
report identifies its period, sources, freshness and missing coverage.

## Access

Doppler is the source of truth for credentials. Inventory and verify access without
exposing values. Financial-reading agents receive only their designated read tools
or narrowly scoped credentials, without inherited credential-administration access.
Provider-enforced read-only permissions must be verified where supported; unsupported
capabilities remain explicit. Credential rotation and administration are separate
from financial reporting authority.

## Direction

Understand overall burn and where compute spending goes. This is not a runway
forecasting product or a general bookkeeping replacement. Start with useful reports
and agent-readable access; choose providers and implementation placement during Task
design. Filing Tasks does not authorize launching workers or connecting accounts.
