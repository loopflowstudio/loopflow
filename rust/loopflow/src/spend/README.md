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
Each interval includes the account, resource,
source, access requirement, credential and environment metadata then in effect.
Changes to linked records create intervals even when the dependency itself is omitted.
Same-date imports use the last supplied record; original snapshots remain retained.

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
`executed_home`, `candidate_version`, `inventory_revision`, `success`, `observed_at` and non-secret
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
Run `lf auth access rotation reconcile <rotation-id>` after changing requirements,
Home bindings or candidate metadata. Reconciliation increments `inventory_revision`,
retains earlier receipts as history and requires fresh receipts for the new revision.
It also clears the complete-consumer attestation; supply fresh
`--consumer-inventory-evidence` before retirement. Consumers referencing either
credential ID are included. Repeating reconciliation without changes retains fresh
receipts. After activation, the original retirement reference stays fixed. An active
key changed outside this rotation must be restored to its recorded reference before
reconciliation; reconciliation never switches keys or revokes them.

Synthetic endpoint tests demonstrate creation, provisioning, failed replacement,
all-consumer verification, restart, cutover and failed/successful retirement, with
runtime secrets absent from persisted records and command output. Provider permission
probes, production provisioning and live acceptance remain open. No CLI operation
creates or revokes provider keys.


```sh
lf auth source import-aws-cur aws-billing --period 2026-09 --file export.json --json
```

Import a finalized legacy AWS Cost and Usage Report downloaded locally, with an
independent invoice total. `export.json` supplies:

```json
{
  "account": "aws-account",
  "invoice_id": "provider-invoice-id",
  "currency": "USD",
  "invoice_total": "90.00",
  "fetched_at": 1791244800,
  "generated_at": null,
  "manifest": "Manifest.json",
  "directory": "downloaded-report"
}
```

Paths resolve relative to the descriptor. Preserve the manifest's `reportKeys`
paths beneath `directory`; every part must be present. Import the account's native
AWS identity and resource identities first. Use the same billing source for other
transports of that invoice. This operation reads local files only.

The reader selects the invoice ID from all manifest parts, validates account,
period and currency, and reconciles exact unblended costs to the supplied invoice
total before publication. Credits, billed refunds, taxes and Savings Plan offsets
are included once; payment transactions are not imported. Unknown charge types,
missing resources, missing parts and unexplained differences preserve prior evidence.
An empty export cannot establish zero cost. The operator supplies the downloaded
assembly and independent bill; the importer does not authenticate their provenance.

This reader supports standalone accounts and daily/monthly UTC intervals in CSV
or gzip CSV, bounded to 64 MiB decompressed per import. Consolidated member-account
billing, hourly intervals, ZIP, Parquet and network fetching remain unimplemented.
It does not treat provisional rows with blank invoice IDs as bills. See AWS's
[billing fields](https://docs.aws.amazon.com/cur/latest/userguide/billing-columns.html),
[line-item fields](https://docs.aws.amazon.com/cur/latest/userguide/Lineitem-columns.html)
and [manifest contract](https://docs.aws.amazon.com/cur/latest/userguide/understanding-report-versions.html).

Attribution rules may specify `project` and `tags` alongside account/resource.
All specified selectors must match dated resource observations over the entire
charge interval. A changed project/tag within the interval or missing prior
observation leaves the charge unresolved; the importer never applies today's tags
to an earlier charge. Explicit resource mappings need no project/tag observation.

Reports include `session_usage` links from the existing retained-input history
reader. These are inputs captured in the selected month, with their recorded
repository/Wave/Task ownership and usage coverage; they are not usage apportioned
into a billing month. Estimated USD and tokens remain separate from invoice totals.
Provider-key linkage remains unknown. Designated exports omit Session evidence.
