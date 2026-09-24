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

## Mutation-child compression review at `43e762b077`

No executable reduction selected. Before and after this review, the model is:
an installed obligation owns original due opportunities; an opportunity owns
attempts, and an attempt owns selection, verification, and product outcome.
Collapsed opportunities reference their execution owner. Atomic `settle` writes
product evidence; physical cron receipts retain process evidence. Publisher
receipts retain preparation/publication stages and fresh public observations.
Cron attribution, release-target exclusion, and worktree protection have
separate capabilities. Child inheritance extends the existing release lock's
lifetime without creating another executor or durable state object.

Inspected the model path from `CronExecution` validation and obligation/attempt
types through settlement and history qualification, then through release
run/tag/publish entry points, candidate completion, `ReleaseLock` acquisition
and child command construction. Checked the worktree lease owner and removal
path as the adjacent authority. The complete Task diff was available without
truncation (496,432 characters); this review concentrated on those owners and
the latest mutation-child change.

Mirrors checked: hidden CLI receipt/descriptor arguments, mechanical flow
dispatch, release history/disposition commands, the release-history JSON
fixture, Python artifact/public receipts and descriptor propagation in publisher,
website deployment, and release scripts. Reviewed the new parent-death fixture
and the release README, cron host guide, release-evidence direction, full design,
and preceding review receipts. Searches found no Swift consumer of the release
history or publisher receipt types.

| Suspected reduction | Why it is not a meaningful reduction here |
| --- | --- |
| `ReleaseLock` and `WorktreeLease` | One excludes a repository/target's release mutations; the other protects a particular checkout, including ordinary removal. Unifying them would alter scope and would not solve surviving stage ownership. |
| `CronExecution` and the release descriptor | The receipt plus job descriptor authenticates scheduled attribution. The target descriptor excludes publication mutations for manual and scheduled callers. A shared capability would conflate those authorities. |
| `ReleaseLock.inherited` | This records validated descriptor reuse and suppresses a false manual intervention for nested publisher calls. Removing it or deriving it from an environment value changes provenance. |
| `command`, `inherit`, and locked/unlocked output helpers | These are private construction/output mechanics, with one OS lock owner and one output/error conversion. Inlining a helper is a small syntax cleanup; introducing an optional lock everywhere or a generic executor adds machinery without reducing the model. |
| Descriptor forwarding in three Python scripts | Each forwards the same OS capability across a real process boundary. Extracting a shared launcher utility only relocates a few lines and couples independently invoked scripts; it removes no domain concept or writer. |
| Frozen `covered` keys and current `coalesced_into` links | Attempts retain historical coverage; links describe current catch-up ownership. The timing and intervention regressions consume both. Deriving historical coverage from current links would lose evidence. |
| Opportunity wait and deferred attempt | A due time can wait without an execution attempt. A deferred attempt records an execution's actual stop. Collapsing them would require synthetic attempts or discard continuation evidence. |

The earlier selection/outcome, process/product, and stage/public-proof distinctions
still hold. Negative searches found no restored `PublicationEvidence`,
`NoChangeEvidence`, `record_verification`, separate Python candidate/publish
receipt classes, or release-selection `sync_main` path. Historical schema-1
receipts remain explicitly required compatibility, not a removable fallback.

No API, route, DTO, field, persisted format, or executable code changed. No tests
or static checks were rerun for this documentation-only review. The prior focused
passes remain their recorded evidence. Shared PR/hook/worktree child exclusion,
telemetry linkage/recovery, closed-obligation continuation, and configured
acceptance remain open; this compression review does not approve the branch.
