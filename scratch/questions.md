# LOO-389 remaining inputs — October 6, 2026

The [approved design](track-computing-dependencies-their-access.md) owns scope,
CLI placement, discovery evidence and acceptance. Remaining inputs:

- Reconcile account identities, duplicate-account relationships and service gaps
  from metadata. Visibility outside the six discovered Doppler projects is unknown;
  secret names alone establish neither accounts nor permissions.
- Resolve the researched provider contract before selecting the first reader:
  Runpod's current public API provides usage buckets, not invoice identity/finality.
  Choose a provider invoice/export contract or represent usage-ledger evidence
  separately; never label those buckets settled invoices. The design records
  official sources and the exact missing evidence.
- Live acceptance requires an explicitly authorized provider account, exact Doppler
  reference and billing period, or an authorized billing export with account and
  period. `loopflow/dev_billing` and admin-key names confer no authorization to
  fetch values or connect accounts. Fixtures cannot satisfy this criterion.

The local core now includes dependency relationship history, access evidence,
designated exports and administrative rotation receipts, with synthetic provider
and Docker isolation proofs. Remaining local work is listed in the design:
provider probes/execution, production provisioning, inventory-drift recovery during
rotation, historical linked-record projections, project/tag attribution and Session
usage linkage. No provider reader or live financial import exists. Mercury
connectivity remains outside this Task.

Reversible choices in this implementation: dependency relationship intervals are
read from existing immutable import snapshots; same-date corrections use the last
import, while original snapshots remain stored. Designated exports disclose only
matching evaluated amounts and opaque source/rule references, excluding full
invoices and free-form coverage. Receipt import is an administrative assertion,
not independent provider verification. Synthetic HTTP endpoint evidence does not
establish production provider execution. These choices preserve full acceptance.

Reversible implementation choices: normalized exports reuse a stable source across
transports; repeated superseded revisions do not reactivate them. Allocations use
the charge's declared decimal precision and deterministic residual assignment.
Available local paths resolve through CanonicalRepo; external identities use RepoId.
These choices preserve historical attribution and do not relax Task acceptance.
