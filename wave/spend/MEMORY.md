# Spend memory

## Dependency inventory — October 6, 2026

Jack Heart clarified that computing dependencies organize both credential hygiene
and spending. Dependencies, provider accounts, Doppler credentials and billing
sources are not one-to-one. Track consumers, access and cost at the depth each
service supports; scalable cloud and AI services warrant detailed usage/cost
breakdowns and granular keys. Unknown cost does not exclude a dependency.

Jack requested combining the credential-management and infrastructure-spending
Task drafts into “Track computing dependencies, their access and their costs.”
The supplied October 6 launch now identifies the combined Task as LOO-389 in
Spend's chapter. It supersedes the earlier filing-blocked observation; the
original draft file is not present in this checkout. Jack authorized launching
only the combined dependency Task. This launch supplies no evidence that the
LLM cost/performance or restricted-agent/bank work has started.

LOO-389 kickoff selected DigitalOcean as the first proposed compute importer,
based on its documented narrow billing scope and invoice/resource detail;
this is an engineering choice, not Jack's account-connection approval. Account
availability and live billing acceptance remain unverified. Invoice items lack
a documented stable line ID: use complete invoice revisions and preserve
corrections, rather than inventing cross-revision line identity. See the
[billing API](https://docs.digitalocean.com/platform/billing/reference/api/).

Restricted reporting must not inherit the general SSH credential bundle.
Environment scrubbing alone does not isolate host credential files. Prefer
isolated report tools without provider/Doppler access. Doppler service tokens
are config-scoped and can have write permission; online verification must not
mistake cached fallback secrets for current access. A successful provider read
does not establish read-only permissions. These are design constraints, not
completed isolation or rotation evidence.

## Founding direction — October 6, 2026

Jack Heart named and requested the Spend Wave and initial Tasks in the Product
conversation. Spend is a peer Wave with cross-repository scope. Its primary views
are total spending, repository, and Wave within a repository, with Task,
Session and resource drill-down. Same-named Waves in different repositories remain
distinct.

Jack joined four responsibilities: the token usage/performance work explored during
summer and fall; better Doppler key provisioning and rotation; non-LLM compute
spending; and restricted agents with read-only access to bank information. Basic
bank financials establish overall burn. The emphasis is scalable software costs,
not runway forecasting. Banks, countries, LLM accounts and compute providers have
not been selected; no account connection or rotation is authorized by filing.

Direct attribution and explicit allocations remain distinguishable. Preserve
historical ownership and show shared/unassigned amounts. Reconcile usage estimates,
provider bills, credits/subscriptions and actual payments without counting the same
expense twice. Restricted financial agents must not inherit broad Doppler access.

Existing evidence to recover before rebuilding metrics: Intelligence memory's
October 1 context-cost instrument (LOO-348), launch-context ablation (LOO-349), and
bind-attribution comparison (LOO-336); `performance/context-cost.md`,
`performance/context-ablation.md`, `performance/bind-attribution.md`, and the existing
usage/history readers. These references are starting evidence, not a claim that
current installed behavior or cross-repository billing is complete.
