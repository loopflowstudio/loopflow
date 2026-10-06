# Dependency access and billed evidence

```sh
lf auth inventory import inventory.json
lf auth source import fixture --period 2026-09 --file invoice.json
lf auth dependency show workers --period 2026-09 --json
lf auth access show laptop --json
lf auth report --period 2026-09 --repo example/one --json
lf auth dependency history workers --json
lf auth export --period 2026-09 --repo example/one -o report.json
```

Use the [inventory](../../../../tests/fixtures/dto/spend/inventory.json) and
[invoice](../../../../tests/fixtures/dto/spend/invoice.json) fixtures as input
examples. They contain synthetic metadata, never credentials. Import only
non-secret JSON. Unknown fields are rejected. Imports upsert named records in
one transaction; omitted records remain. Include the complete current discovery
gap list in each inventory import. A source’s account list replaces its current
scope; removing an account preserves its imported invoices and rejects new documents
for that account.

Name local repositories by checkout path; imports resolve existing paths to the
canonical checkout. Name external repositories by `owner/repo`. Wave references
are existing Wave IDs and must match their repository. `--wave` also takes a Wave
ID, keeping same-named Waves separate. Historical absolute repository paths remain
usable when the checkout is unavailable.

Dependencies may share accounts, sources, resources and credential requirements.
Account links expose invoices as related evidence without assigning their costs.
Unknown attributed cost is `null`. A complete zero invoice contributes zero;
a missing period has no total and an explicit gap. Subscription declarations are
estimates, displayed separately from charges.

Invoice identity is source, account and provider document ID. Use one stable
source authority for API and export transports. Complete corrections replace the
current revision atomically, preserving prior evidence. Reordering identical
items is idempotent; identical lines retain their multiplicity. A repeated old
revision does not undo a later correction. Decimal amounts are strings, with no
currency conversion. Reconciliation differences remain visible.

Explicit rules assign charges to repositories, Waves or dependencies. Allocations
use basis points and the charge's decimal precision, truncate each share toward
zero, and assign the residual to the final recipient of a complete allocation.
Partial allocations leave their residual unassigned. Conflicting rules and
charges crossing an effective boundary remain unassigned. Rule revisions and
invoice evaluations are immutable; editing a mapping cannot change a prior
invoice revision. Give changed rules and access requirements new revision IDs.

Inspections open a read-only Store connection and never resolve credentials or
contact providers. The CLI's normal Exec ledger remains separate from that read
connection. Reports include source revisions, timestamps and coverage gaps.
`lf account` continues to own login identity and routing.

```sh
lf auth access verify laptop --period 2026-09 --json
```

Verification binds observations to the executing Home, requirement revision and
available credential version and exact Doppler reference. Dependency inspection
includes the same current evidence and dated observations. The implemented probe reads the local `auth.report`
tool only when the environment names this Home. It does not certify isolation or
provider-enforced permissions. Remote environments and provider credential probes
return unavailable evidence. Previous observations remain as dated history.

`dependency history` reads effective intervals from committed inventory snapshots.
Each version contains the dependency's consumers, accounts, resources, sources and
requirement links, with provenance. Omitted dependencies retain their previous
relationships. For same-date corrections the last import wins in the effective
view; original import snapshots remain stored. `dependency show` uses current
metadata regardless of the billing period; history does not restate invoices.

Report filters narrow totals but retain administrative invoice evidence. Use
`auth export` for a designated consumer: it requires a repository and optionally a
Wave, and includes only matching evaluated amounts, rule revisions, and source
references. It omits invoice totals, free-form descriptions, other recipients,
credential metadata and unrelated coverage. Its totals are partial attribution;
reconciliation and completeness still require administrative inspection.

```sh
uv run python scripts/check_spend_isolation.py
```

The headless check seeds synthetic invoices in a temporary Home, exports one
recipient and mounts only that read-only JSON into a container. It checks the
amount, denied writes and absence of administrative paths, Doppler, SSH sockets
and inherited authority. It requires Docker and a Python-equipped image
(`rust:bookworm` by default). This fixture demonstrates the export boundary;
production agent provisioning remains separate work.

```sh
lf auth access rotate key --replacement candidate
lf auth access rotation receipt <rotation-id> candidate-read.json
lf auth access rotation activate <rotation-id>
lf auth access rotation receipt <rotation-id> consumer-cutover.json
lf auth access rotation receipt <rotation-id> retirement.json
```

Import both credential references before starting. Rotation records contain only
metadata and provider/operator receipts. A receipt supplies `operation`
(`candidate_read`, `consumer_cutover` or `retirement`), `environment`,
`executed_home`, `candidate_version`, `success`, `observed_at` and non-secret
`evidence`. Retirement receipts use null environment/Home. Treat receipt import
as administration: it records the operator's evidence and does not independently
verify it or execute a provider action.

Every required environment must acknowledge the candidate before activation.
Activation changes the original credential ID's active reference transactionally;
existing requirement/source links keep that ID. It does not reload consumers.
Every consumer then needs a cutover receipt dated after activation. Retirement
also requires explicit `--consumer-inventory-evidence` when starting or resuming
`rotate`; unknown consumers never imply permission to retire. Failed retirement
keeps the new reference active and the old key explicitly pending retirement.
`rotation show` resumes inspection; repeating activation preserves that state.
`rotation cancel` abandons a candidate before activation without revoking anything.
Changed credential metadata or consumer requirements need reconciliation before
continuing; current receipts cannot certify a changed inventory.

Synthetic endpoint tests demonstrate creation, provisioning, failed replacement,
all-consumer verification, restart, cutover and failed/successful retirement, with
runtime secrets absent from persisted records and command output. Provider billing
readers, provider permission probes, Session usage linkage, production provisioning
and live acceptance remain open. No CLI operation creates or revokes provider keys.
