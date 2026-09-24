# Release evidence reduction

## Model and proof boundary

The effective model is an installed obligation with original due opportunities.
An opportunity owns append-only attempts; each attempt owns its selected
candidate, verification, and product outcome. Coalesced opportunities refer to
one owner. Physical cron receipts describe execution, never publication.
Publisher artifact receipts retain exact identity, hashes, and completed stages;
public read-back adds download and smoke observations.

The finish line for this pass is one verification authority per attempt and one
atomic settlement writer, with unchanged failure history and pair qualification.
A green publisher fixture or two invented process successes do not count as
configured acceptance.

## Reduction

Before: successful outcomes carried a second verification list inside
`PublicationEvidence` or `NoChangeEvidence`. The runner wrote attempt checks
through `record_verification`, then saved the outcome separately. History judged
the nested copy, while failure and attempt inspection used the parent copy.
After: `ReleaseAttempt.verification` owns all check references. `settle` saves
that list and the outcome under one accounting lock and atomic document
replacement. Outcomes carry only their product facts, directly on the enum.
Conflicting settlement diagnostics retain both proposed outcome and proof;
idempotence compares both against accepted evidence.

Removed the two evidence wrapper types, both duplicate verification fields,
the `record_verification` writer, and successful outcome `evidence` envelopes.
Updated the release producer, history qualification/deduplication, accounting
preservation cases, and JSON fixture together. These new obligation/history
formats have not been installed; no compatibility decoder or migration is added.
Historical schema-1 physical cron receipts remain unchanged.

Python preparation and publication previously returned identically shaped
`CandidateReceipt` and `PublishReceipt`. Both now use `ArtifactReceipt`.
Completed stages and retained file locations still distinguish observations at
each publication boundary. Existing receipt JSON is byte-shape compatible.
`PublicReleaseReceipt` remains a subtype because it adds independently observed
public URLs, verification time, platform, and actual smoke versions.

## Mirrors inspected and retained concepts

- Core/storage: accounting structs, file replacement, selection/settlement and
  rejected-input writers; release run/recovery; publisher receipt stages.
- Public interfaces: release history JSON/text, disposition, cron receipts,
  release run return type, explicit cron context and inherited release lock.
- Mirrors: Rust history readers, release producer, Python publisher/tests, the
  release-history JSON fixture. Repository search finds no Swift consumer of
  these release-history or artifact receipt types.
- Documentation: release README, cron host guide, repository release-evidence
  direction, and the full scratch design.

Keep selection separate from outcome: selection survives a failed attempt before
publication exists. Keep physical process status separate from product status:
a wrapper can fail after verified publication. Keep original installation,
segment activation, and observation timestamps: reconfiguration and unknown
historical timezone coverage give them distinct meanings. Keep predecessor
links and frozen covered keys: they preserve crash recovery and catch-up
cardinality. Keep intervening failures and manual provenance after successful
retry. Keep `ReleaseRunOutcome`: manual runs return operation results without
claiming a configured scheduled settlement. No scheduler or PM behavior changes.

The review rejected a generic current-attempt mutation abstraction: target,
selection, and settlement have different immutability rules. Removing the extra
verification writer removes the relevant duplication without hiding those rules.

## Preservation proof

The accounting test now retains nonempty proof, rejects replacement of that
proof even with the same product outcome, checks the unchanged accepted record,
and inspects retained rejected input. History cases additionally reject a failed
check and an unverified retry after a checked attempt. The fixture round-trip
keeps the same summary and failed telemetry/late disposition observations.
Publisher tests exercise the existing candidate-to-public read-back path with
one receipt type, retaining rejection of missing checks or wrong assets.

Validation results are appended after completion. This pass does not install,
sync, trigger, publish, assign repair work, or establish the two-settlement KR.

## Review and validation

The simulated review found that flattening the outcome alone would still leave
two persistence boundaries. Joining verification to `settle` removes that gap
and makes its immutable comparison cover the complete accepted evidence.
Rejected proof is retained with the rejected outcome, so removing the nested
copy loses no diagnostic input. The retry test uses a distinct receipt after a
failed attempt, rather than creating an impossible retry after accepted success.

- Eight accounting preservation tests passed, including atomic evidence,
  conflicting proof, late wrapper failure, candidate retry, and delayed coverage.
- Two history tests passed with unchanged summary/failed telemetry/late ownership
  and additional missing, failed, and previous-attempt proof counterexamples.
- Ten publisher tests passed. External services remain mocked; this is not live
  artifact, signing, or UI evidence.
- Rust formatting and all-target Clippy passed; Python Ruff check/format passed.
- No affected-suite gate or full CI run was added. The scorecard schema blocker,
  real UI-host proof, and two configured automatic settlements remain open.

## Follow-up compression review at `4a8a9f312`

No further executable reduction selected. The effective model remains:
an installed obligation owns original due opportunities; coalesced opportunities
refer to one execution owner; each attempt retains selection, verification, and
outcome. Atomic settlement owns product truth. Physical process receipts,
publisher stage evidence, and fresh public observations retain their separate
meanings. The joined execution and publisher-recovery changes did not introduce
another settlement writer.

Reviewed the accounting types and `begin`/`finish_process`/`settle` writers,
history qualification and disposition, release selection/completion, publisher
receipts and recovery, and explicit cron/lock context. Followed these into CLI
flow dispatch, catalog selection, release output, history JSON, the DTO fixture,
the joined scheduled-release test, publisher counterexamples, release README,
cron host guide, and release-evidence direction. Search found no Swift consumer
of these release-history or publisher receipt types.

| Suspected duplication | Why it remains |
| --- | --- |
| Selection and successful outcome both name a tag/commit | Selection exists before success and survives preflight or publication failure. Deriving it from settlement would erase recovery identity. |
| Physical receipt and attempt both describe execution | Wrapper exit and release settlement can disagree. Their completion timestamps describe different boundaries; historical schema-1 receipts remain required evidence. |
| Manual `ReleaseRunOutcome` and scheduled outcome | Manual results report selection/recovery and CLI output. Scheduled settlement additionally requires current telemetry and public proof. Merging them would overstate manual evidence. |
| `ArtifactReceipt`, `PublicReleaseReceipt`, Rust `PublicReleaseProof` | Preparation/stage evidence and public observations differ. Rust validates the publisher's identity and required stages while retaining the complete JSON; it does not reimplement artifact download or smoke. |
| Publisher `verify` and `reconcile` | Both enter the same verification function. Only reconciliation may repair missing/stale stages; collapsing their public commands would change side-effect permission. |
| Initial public checks and post-repair read-back | The former selects permitted repairs; the latter proves external availability after mutation. Reusing initial observations would accept stale proof. A generic stage runner would add machinery around four concrete operations. |
| Job, accounting, and target locks | They protect execution attribution, short atomic record updates, and release mutation respectively. Merging them would change lock scope and ordering, while leaving the missing mutation-child inheritance unresolved. |
| Catalog target and installed target kind | Discovery holds executable content; installed kind records launch provenance. Declarative cron's Flow requirement and general direct target selection have different contracts. |

The local `ReleaseArtifacts` argument bundle and repeated JSON/file-writing
mechanics offer small helper cleanups, but no competing domain authority or
public representation to remove. They do not justify a structural change in
this pass. Negative searches confirm the earlier duplicate evidence wrappers,
`record_verification`, separate candidate/publish receipt classes, and release
selection's `sync_main` path remain absent from the inspected production files.

No API, DTO, persisted field, or executable code changed; no tests or static
checks were rerun. Earlier test results above remain historical evidence, not
new validation. The review's iterate verdict and the follow-up's telemetry
linkage/retry, mutation-child exclusion, closed-obligation continuation, and
configured acceptance obligations remain open. This review does not establish
two automatic settlements or resolve the observed scorecard schema blocker.
