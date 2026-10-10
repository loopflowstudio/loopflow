# Automatic storage cleanup

Direction from Jack Heart, 2026-10-09: “lets finish following up as much as we can
on loopflow cleanup automatically for all users”. This extends product prevention,
not permission to delete arbitrary customer data. The earlier request made Etude
experiment output disposable; it did not make every customer's ignored file
disposable or authorize expiration of conversation history.

## Outcome and boundary

Ordinary installed Loopflow cleans up finished work and bounds its own disposable
storage without an agent, repository-specific test script, or recurring manual
prune. It explains retained bytes and failures. Active work, source, credentials,
recoverable conversations and rollback state survive.

Infrastructure owns this additive series. The current `clean-up` branch owns the
worktree collector; `scratch/clean-up.md` owns its exact design and remaining
acceptance. No second collector, scheduler, Task status or artifact registry is
needed. Its publication is not proof that an installed release cleans a machine.

[The earlier storage investigation](storage-footprint.md) and
[LOO-390](https://linear.app/loopflow/issue/LOO-390/reduce-loopflows-disk-footprint-and-explain-its-growth)
remain the baseline; already-merged database, backup and installation work is not
reimplemented here. This file records remaining product outcomes, not a ledger
of local disk deletions.

## 1. Finish automatic checkout collection — current PR

The shared collector now serves terminal Task/PR cleanup, manual preview and the
existing per-repository maintenance tick. New and upgrading installations need
schedule activation/repair automatically; explicit disable survives upgrade.
Unsupported schedulers get the documented foreground fallback, not an idle-time
promise. Experiments must not install production services.

Remaining: stream fresh safety evidence so large healthy histories do not hit the
same whole-set timeout forever; resolve the separate unknown-alias locality
contract without interpreting unknown as safe; run composed gate and installation
proof. Final observation uses a no-progress deadline, not a larger fixed timeout
or persisted negative filesystem observations. Healthy finite work can take longer
than the admission window; stalls retain safely and report why.

The demo is a real Process exiting after a finished Task: the configured tick then
removes its checkout without another agent. An unfinished neighbor, post-merge
commits, ignored personal files and referenced native history remain intact.
All source/history preservation conditions in the exact design remain binding.

## 2. Bound diagnostics and interrupted temporary output — LOO-390 follow-through

LOO-390 already owns storage prevention; reuse it rather than file a competing
storage umbrella. Customers should not acquire unlimited operational logs or abandoned temporary
payloads merely by running lf. Audit current writers and their actual readers,
then give disposable output one retention policy exercised by the same maintenance
entry point. Legacy directory names alone are not ownership or disposal evidence.

Proposed implementation defaults, not a claim of Jack's explicit approval:
seven days uncompressed, 30 days total, and a 4 GiB machine-wide disposable
**diagnostics** budget. Under byte pressure, discard eligible oldest diagnostics
before their age limit. Active writers and sole evidence are excluded; if protected
bytes exceed the target, report that fact instead of promising a hard cap. Bound
individual active log segments at the writer, not by unlinking an open log and
letting it consume hidden disk. Configurability must not create per-Session budgets
that multiply without bound.

Temporary output needs provenance at creation and reclamation after success,
failure and interruption. Scan only lf-owned roots, protect live/unknown owners,
and never sweep all of `/tmp` or another provider's scratch. Reference preservation
uses the history owner; it must not copy the checkout collector's evolving evidence
logic. Existing two-generation migration-backup and install-artifact retention
remain their owners' responsibility.

Delete — do not maintain: unbounded disposable-log writers and duplicate ad hoc
retention for the same owned output, identified concretely during this follow-up's
design. Keep capture/native readers, credentials, unknown legacy backups and
unique history. Historical `traces` are not automatically diagnostic garbage.

Acceptance: fake-clock age/byte tests plus real writer interruption and repeated
maintenance show bounded disposable growth, active-writer preservation, no symlink
escape and idempotent retry. New/upgrade installed paths invoke the same policy.
Inventory reports allocated estimates, eligible/protected/unknown bytes and reasons;
actual free-space deltas are measured separately. No live customer home in tests.

## 3. Evict cold artifacts without deleting unfinished source — after core contract

Worktrees that must remain can still hold tens of GB of regenerable or explicitly
disposable output. Add declarations for disposable roots, idle/age/byte budgets
and explicit retention. Tool cache tags can identify caches; arbitrary names and
`.gitignore` cannot. Experiment results such as Etude `.runs` can be declared
expendable even if they cannot be reproduced byte for byte.

Protect running producers **and consumers**, source, selected retained results and
Task identity. Cover persistent checkouts and proven interrupted-removal residue.
Budgets apply across a machine, not once per worktree. Avoid repeatedly evicting
hot builds only to recreate them; report an active working set larger than budget.
Default cold age and artifact-byte/free-space targets need the actual artifact
contract and workload measurements, not a promise that every machine fits 500 GB.

Replace the duplicated build-cache eviction in `scripts/resource_envelope.py`
with this product owner while retaining that script's test resource admission.
Proof: over-budget idle caches shrink across repositories while active builds,
unfinished source and explicitly retained experiment results survive. The same
behavior runs from a customer installation without this repo's scripts.

This follow-up remains named, not launched, until the shared collector's evidence
and activity contracts settle. It must not build a rival cleaner while they change.

## 4. Reduce historical storage without losing conversations — separate follow-up

Lossless cold storage for lf-owned completed captures is permitted to be designed;
conversation expiration is not selected. Keep SQLite's durable identities, outcomes,
usage and reference ownership. Make archive reads transparent to existing history
commands; prove every consumer before retiring duplicate payloads. Compression
must use atomic publication, integrity verification and retryable interruption.

Provider-owned transcript/SQLite layouts are not lf-owned disposable caches. No
blind gzip, SQLite/WAL removal or undocumented edits. Their compaction needs a
supported, tested restore/resume contract first. Unknown legacy traces/backups
stay protected or are archived losslessly with working readers, not discarded by
age. Archive corruption/missing payload must report an error, not erase identity.

Acceptance compares history/final answers and supported resume paths before/after
archive, including interruption. Measure allocated bytes reclaimed; compression
alone is not a bound on indefinite history growth. Any future expiry decision is
explicit and separate.

## Delivery and measurement

Keep one serial implementation lane; finish the core rather than start several
cache-building pursuits. Diagnostics can be designed independently, but integration
uses the existing maintenance owner. Artifact eviction depends on its settled
contract. No new Task duplicates an existing owner merely to track a checklist.

[LOO-433](https://linear.app/loopflow/issue/LOO-433/remove-superseded-install-binaries-and-app-bundles-when-an-install-settles)
already owns installed proof for merged install-artifact retention; preserve that
ownership. Verify fresh install, upgrade, persisted disable, scheduled retry after
exit, sleep/wake and unsupported-host behavior in isolated headless environments.
Required commands and fixtures belong to each implementation's gate, not a live
machine experiment. Release and installed proof are still required before saying
this protects all users.

Measure disposable/protected/unknown allocated bytes and oldest eligible age,
observed free-space change and repeated-pass growth under a representative workload.
Directory totals can overlap via APFS clones or hardlinks; never add them into a
claimed guaranteed reclaim amount. Personal apps, Messages, Dropbox and Docker
volumes are outside automatic lf deletion.
