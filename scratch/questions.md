# LOO-389 remaining inputs — October 6, 2026

The [approved design](track-computing-dependencies-their-access.md) owns scope,
CLI placement, discovery evidence and acceptance. Remaining inputs:

- Reconcile account identities, duplicate-account relationships and service gaps
  from metadata. Visibility outside the six discovered Doppler projects is unknown;
  secret names alone establish neither accounts nor permissions.
- The local reader selects finalized AWS CUR exports with manifest completeness
  and an independent invoice control total. Discovery has not established an AWS
  billing account. Initial integration gaps include hourly/consolidated billing
  and ZIP/Parquet; match any authorized evidence to supported semantics before
  import. Runpod usage buckets remain distinct from settled invoices.
- Live acceptance requires an explicitly authorized provider account, exact Doppler
  reference and billing period, or an authorized billing export with account and
  period. `loopflow/dev_billing` and admin-key names confer no authorization to
  fetch values or connect accounts. Fixtures cannot satisfy this criterion.

The local core now includes dependency relationship history, access evidence,
designated exports and administrative rotation receipts, with synthetic provider
and Docker isolation proofs. Remaining local work is listed in the design:
additional provider probes/execution, SSH server integration coverage,
and broader acceptance coverage. Dated CLI import outcomes and a reusable isolated
export consumer now exist. `access verify --export` now binds a directly collected
local container receipt to the Home and requirement scope. `auth.report` remains
administrative-only. The requested remote designated-delivery cut is implemented, alongside the earlier
local integration and Runpod read probe. SSH server integration coverage and
additional provider probes remain separate work; neither requires repeating those
completed implementations.
Rotation reconciliation, historical linked
metadata, project/tag rules, separate Session usage links and a local AWS export
reader now exist. No live financial import exists. Mercury
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

The AWS reader is a reversible local implementation choice, not account selection
or connection authorization. Its descriptor supplies the independent bill total;
no downloaded assembly or operator receipt is independently authenticated. Session
links select captured inputs, not monthly usage slices. Historical access metadata
does not assert historical read success.

Import outcomes describe completed CLI attempts at source/period scope; latest
success does not prove every document in a source is complete. Raw errors and
paths are deliberately excluded. Recording failure after invoice commit is
reported explicitly, without claiming the invoice rolled back. The shared Rust
container reader is an operator-run designated tool; its local
process receipt is not a remote agent's authenticated receipt or authorization to
connect a provider. The reversible local choice trusts the administrator's Docker
daemon and preinstalled image, pins a configured Unix socket and uses a private
child-process pipe. No uploaded receipt can establish success.

The reversible first access-probe choice is Runpod billing history: its documented
fixed GET can establish read access without importing buckets as settled invoices.
AWS CUR remains the selected invoice reader. `billing_probe: runpod` requires an explicit
Doppler credential reference and local Home; it accepts no endpoint override.
No account/reference/period authorization is inferred from this implementation.
A successful read retains unknown account identity, read-only scope and actual key
version; inventory versions are declarations until independently verified.

October 6 reconciliation closes the preceding billing-read implementation request.
Its recorded component checks do not establish a successful Doppler-to-provider
CLI run. Broader integration coverage belongs to gate; SSH server authentication remains an integration-check gap and live billing acceptance still requires
the explicit inputs above.

The remote export implementation uses stored Home routes and preprovisioned SSH
identity/known-host entries, never the general credential-forwarding bundle.
A same-channel Home handshake precedes export transmission. The administrator
trusts the remote controller and Docker host; only its isolated reader receives
report data. The headless controller fixture proves consumption with synthetic
Home identity and real Docker, not actual SSH server authentication or live access.
Default OpenSSH identity locations are the reversible initial choice; no new
credential registry, SSH config inheritance or enrollment authority was added.
