# Dependency access and billed evidence

```sh
lf auth inventory import inventory.json
lf auth source import fixture --period 2026-09 --file invoice.json
lf auth dependency show workers --period 2026-09 --json
lf auth access show laptop --json
lf auth report --period 2026-09 --repo example/one --json
```

Use the [inventory](../../../../tests/fixtures/dto/spend/inventory.json) and
[invoice](../../../../tests/fixtures/dto/spend/invoice.json) fixtures as input
examples. They contain synthetic metadata, never credentials. Import only
non-secret JSON. Unknown fields are rejected. Imports upsert named records in
one transaction; omitted records remain. Include the complete current discovery
gap list in each inventory import.

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
available credential version. The implemented probe reads the local `auth.report`
tool only when the environment names this Home. It does not certify isolation or
provider-enforced permissions. Remote environments and provider credential probes
return unavailable evidence. Previous observations remain as dated history.

Provider billing readers, Session usage linkage, credential rotation and isolated
report provisioning remain unimplemented. The normalized export importer is a
local invoice core, not a provider integration or live acceptance demonstration.
No command here creates, replaces or revokes provider credentials.
