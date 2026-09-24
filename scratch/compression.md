# Release evidence reduction

> Evidence correction at `8bb7eca0cedb3845def0017da3496c948a160040`:
> earlier sections missed indirect `sync_main` calls through worktree creation.
> The hook implementation reproduced and removed those calls; see
> [release-hook-ownership.md](release-hook-ownership.md). Earlier negative
> searches establish only their inspected coverage, not complete reachability.

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

## Publisher checkout compression review at `5ca558676`

No executable reduction selected. The model before and after this pass is
unchanged: an obligation retains original due opportunities; each execution
owner retains attempts with selection, verification and product outcome.
`settle` atomically writes that evidence. Physical receipts describe process
results, and publisher receipts describe artifact stages and public observations.
Scheduled attribution, target mutation exclusion and checkout removal protection
remain separate capabilities. Child inheritance extends an existing OS lock's
lifetime without adding durable execution state.

Inspected obligation/attempt types, settlement and history consumers, then
followed `ReleaseLock` and `WorktreeLease` through preparation, publication,
public reconciliation and ordinary/owned checkout removal. The complete Task
patch was available without truncation (531,176 characters); this pass focused
on the new checkout preservation path and its direct owners and mirrors.

Mirrors checked: explicit CLI receipt/descriptor arguments and Flow dispatch,
release history/disposition output and its JSON fixture, Python artifact/public
receipt types, descriptor forwarding in publisher/deployment/packaging scripts,
the Rust parent-death fixture, Python descendant and entry-authority cases,
release README, cron host guide and the current design/report. Searches found
no Swift consumer of the release-history, opportunity, public-receipt or
checkout-descriptor names.

| Candidate | Why retained |
| --- | --- |
| `WorktreeLease` beside `ReleaseLock` | Ordinary removal protects one checkout, including non-release work; the target lock excludes release mutations across checkouts. Combining them changes scope and independent-checkout behavior. |
| Lease `path` and `file` fields | The path binds owned removal to the exact checkout; the file holds the OS lock and supplies child inheritance. Neither duplicates release selection or durable history. |
| Two descriptor environment keys | Descendants need both lifetimes. Checkout possession alone cannot authorize publisher entry; replacing the keys with an undifferentiated capability would lose that boundary. |
| Rust inheritance methods and Python forwarding lists | These are short process-boundary mechanics, not competing lock owners. A generic launcher or shared script module would relocate syntax without reducing domain/API vocabulary. |
| Ordinary and owned removal entry points | One acquires protection; the other borrows existing protection and checks its checkout. Collapsing them would require implicit ownership or reacquiring an already held lock. |
| Cron context, attempt evidence and publisher receipts | Attribution, product settlement and independently observed artifact stages still answer different questions. The new lease adds no persisted mirror to remove. |

Negative searches found no restored duplicate success-proof wrappers,
`record_verification`, separate candidate/publish receipt classes or
release-selection `sync_main` path. Historical schema-1 process receipts remain
required history, not a removable compatibility seam.

No APIs, DTOs, fields, storage formats or executable code changed. No tests or
static checks were rerun for this documentation-only pass; the preceding
implementation's focused results remain recorded in
[publisher-checkout-preservation.md](publisher-checkout-preservation.md).
Shared mutation-child exclusion, historical telemetry linkage/recovery and repair
ownership, closed-obligation continuation and configured acceptance remain open.
This review neither approves the branch nor supplies the two automatic settlements.

## Hook ownership compression review at `8bb7eca0ce`

No further executable reduction selected. Before and after this pass, an
obligation owns original due opportunities; each execution owner retains attempts
with selection, verification and product outcome. Collapsed opportunities refer
to that owner. Atomic `settle` writes product evidence; process receipts describe
wrapper execution; publisher receipts retain artifact stages and public
observations. Cron attribution, target mutation exclusion and checkout removal
protection remain distinct capabilities. Hook inheritance extends their existing
lifetimes without another execution record.

Inspected the accounting types and `begin`/`finish_process`/`settle`/receipt-context
path, history timing/qualification/disposition readers, then release run and
selection, verification hooks, preparation/rebuild, target acquisition/inheritance,
checkout acquisition and cleanup. The complete Task patch was available without
truncation (571,234 characters); this pass concentrated on those model paths and
the latest hook implementation. The effective public contract remains one
mechanical release operation plus separate scheduling and product history.

Mirrors checked: explicit cron context through CLI Flow dispatch, manual release
output, history/disposition DTOs and the release-history JSON fixture; Python
artifact/public receipt types and descriptor forwarding in publisher, deployment
and packaging helpers; hook survival/caller-preservation fixtures; release README,
cron host guide, evidence direction and full design. Searches found no Swift
consumer of the release-history, opportunity, public-receipt or checkout-fd names.

| Suspected reduction | Disposition |
| --- | --- |
| Hook arguments wrapped in a new context/runner | Would group syntax without deleting a domain concept or writer. Explicit target lock and checkout lease show which authority reaches the child. |
| Target lock, checkout lease and cron descriptor | Different scopes: repository/target mutation, exact checkout removal, scheduled attribution. Unification changes capability boundaries and independent-checkout behavior. |
| `PreparedRelease` and durable candidate selection | The former is private PR number/head evidence before merge; the latter retains candidate/tag recovery identity. Neither can replace the other without discarding a stage's evidence. |
| Separate rebuild helper and optional cleanup lease | Already removed by implementation. Rebuild is beside its lease owner, cleanup requires ownership, and preparation takes the existing `ReleaseChangeSet`. No remaining alternative representation to remove here. |
| Verification and preparation hook lists | Same runner, different configured phases and source subjects. Combining the lists would change execution order and verification meaning. |
| Physical receipt versus attempt; selection versus outcome | Wrapper exit may disagree with product settlement, and selection must survive before or without success. These are preservation evidence, not duplicate lifecycle authorities. |
| Publisher stage receipt versus public receipt | Preparation/publication effects and fresh external observations have different proof obligations. Flattening away that distinction would overstate completion. |
| Frozen covered keys versus current collapse links | Historical execution coverage preserves timing and provenance across retries; current links identify the owning result. One cannot reconstruct the other after ownership changes. |

All three release worktree-creation calls now explicitly disable default-branch
sync; inspected helper code performs sync only when that argument is true.
This narrower source finding supersedes the earlier missed indirect path.
Searches also found no restored success-proof wrappers, separate verification
writer, or rebuild adapter. Shared PR/notes/source/cleanup subprocess inheritance
remains incomplete; hiding those reachable gaps behind a generic command wrapper
would not establish their ownership contract.

No API, DTO, persisted field, migration or executable code changed. No tests or
static checks were rerun; the preceding implementation's focused results remain
recorded evidence. Only this report and its correction changed. Historical
telemetry association/recovery and repair ownership, closed-obligation
continuation, remaining mutation children and configured acceptance remain open.
The scorecard blocker and required UI/public proof are retained; no PR approval,
publication or two-settlement claim follows from this compression review.

## Auto-merge ownership compression review at `e041f8b45b`

No executable reduction selected. The model before and after this pass is an
obligation containing original due opportunities; an execution owner retains
attempts with selection, verification and outcome. Collapsed opportunities
reference their owner. `settle` atomically writes product evidence; physical
receipts retain process results, and publisher receipts retain artifact stages
and public observations. Cron attribution, target exclusion and checkout removal
protection remain distinct capabilities. Auto-merge inheritance extends their
existing lifetimes without creating another executor or durable state object.

Inspected accounting types and execution validation, `begin`, `finish_process`,
`settle` and receipt context; history qualification and failure dispositions;
release preparation/waiting, shared PR enable/disable, land finalization and
ordinary CLI/Task callers; target-lock acquisition/inheritance and checkout
leases. The complete Task patch was available without truncation (611,209
characters). This pass concentrated on those model paths and the new auto-merge
cut, rather than treating historical reports as fresh verification.

Mirrors checked: hidden cron receipt/descriptor arguments and Flow dispatch,
history DTO and JSON fixture, Python artifact/public receipt types and descriptor
forwarding, auto-merge survival fixture, release README, cron host guide,
release-evidence direction and full design. Searches found no Swift consumer of
`ReleaseHistory`, `ReleaseOpportunity` or `PublicReleaseReceipt`.

| Candidate | Why retained |
|---|---|
| Inheritance callback and `ReleaseLock::command` | The callback configures existing shared PR commands with capabilities held by their caller. The constructor serves direct release children. Unifying them into an executor introduces machinery without removing a domain owner. |
| Separate target and checkout inheritance | Waiting owns target exclusion; preparation also owns a checkout. Combining these capabilities changes removal scope and would force waiting to invent checkout ownership. |
| Ordinary callers' no-op callbacks | They explicitly supply no release capability. A default wrapper or ambient lookup would add an entry point or conceal ownership, not reduce the model. |
| `arm` and `finish_arm_after_rebase` | Required versus completed integration is real behavior. Both use one preparation implementation; collapsing them would either repeat integration or expose its mode publicly. |
| Enable and disable auto-merge | Exact-head arming and idempotent revocation are distinct operations. Replacement composes the existing functions; a mode enum would merely rename their branching. |
| PR number/head arguments in finalization | Passing a borrowed `PrInfo` could group two arguments, but removes no concept or competing representation. That local signature cleanup alone does not meet this pass's structural bar. |
| Selection, outcome, physical receipt and publisher proof | They preserve recovery identity, product settlement, wrapper exit and public observations at different boundaries. Combining them would discard evidence or overstate completion. |
| Frozen coverage and current collapse links | Historical timing/provenance consumes the former; current ownership uses the latter. Neither reconstructs the other across retries. |

Searches found no restored duplicate success-proof types, separate verification
writer or Python candidate/publish receipt classes. The explicit callback covers
auto-merge commands only. Commit/push, PR creation/editing/readiness, notes,
lockfile tools and source/cleanup mutation children remain implementation gaps;
a generic wrapper would not establish their ownership contract.

No APIs, DTOs, fields, storage formats or executable code changed. No tests or
static checks were rerun for this documentation-only review. Prior focused
results remain recorded in [release-auto-merge-ownership.md](release-auto-merge-ownership.md).
Historical telemetry association/recovery and repair ownership, closed-obligation
continuation and configured acceptance remain open. The scorecard blocker,
required UI/public proof and two automatic settlements are not resolved by this
review. No publication, landing or Task completion was performed.

## PR mutation ownership compression review at `95ffe75aab`

No executable reduction selected. The model before and after this pass remains:
an obligation owns original due opportunities; an execution owner retains
attempts with selection, verification and outcome. Collapsed opportunities
reference that owner. `settle` atomically writes product evidence. Physical cron
receipts retain process results; publisher receipts retain artifact stages and
public observations. Cron attribution, target exclusion and checkout removal
protection remain distinct capabilities.

Inspected the accounting types, `begin`, `finish_process`, `settle`, receipt
validation and history qualification, then followed release preparation through
commit, `finish_arm_after_rebase`, `ensure_pr`, shared creation, retargeting,
metadata editing, readiness and auto-merge. Inspected target/checkout inheritance
and the adjacent Task compensation calls. The complete Task patch was available
without truncation (642,657 characters); this pass concentrated on those model
paths and the latest PR mutation change.

Mirrors checked: explicit cron context through CLI/Flow dispatch, release-history
and disposition output, the history JSON fixture and its round-trip test, Python
artifact/public receipt types, the PR mutation survival fixture, release README,
cron host guide, release-evidence direction and current design/report. Repository
search found no Swift consumer of `ReleaseHistory`, `ReleaseOpportunity` or
`PublicReleaseReceipt`.

| Candidate | Why retained |
|---|---|
| Commit draft creation and shared PR creation | The implementation already removed their overlap in release preparation: commit/push supplies no draft; finalization creates the PR. Ordinary CLI and Flow commit callers still request drafts. Deleting that option would remove their behavior. |
| PR edit/readiness helpers in `land` and `pr` | Finalization edits the current branch's PR without changing its base; ordinary publication edits an observed PR number including its base. Readiness likewise has branch and numbered targeting. Combining these short helpers would require optional targeting/update modes or change behavior, without removing a domain owner. |
| `create_pr_from_pushed_branch` and private `create_pr` | Both use one creation command. The former returns the PR identity and local head needed by finalization without repeating Git publication. Inlining it would move that interpretation into its caller, not remove competing creation authority. |
| Inheritance callback and lock command constructor | The callback carries already-held capabilities into shared PR commands; the constructor serves direct release commands. A generic command runner would add an abstraction without completing the remaining child paths. |
| Target lock, checkout lease and cron execution descriptor | They protect different scopes. Combining them would conflate mutation exclusion, exact checkout removal and scheduled attribution. |
| Selection/outcome, process/product, stage/public evidence | Selection exists before success; wrapper exit can disagree with settlement; public read-back establishes more than completed publisher stages. Removing these distinctions would discard recovery or proof. |
| Frozen covered keys and current collapse links | Historical timing uses saved execution coverage; current links identify catch-up ownership. Neither reconstructs the other across retries. |

Searches found no restored duplicate success-proof wrappers, separate
`record_verification` writer or Python candidate/publish receipt classes.
Historical schema-1 process receipts remain required evidence. Task compensation
still uses ordinary revocation calls; this review does not extend the inheritance
claim to those paths or to shared Git, notes, tools and worktree subprocesses.

Only this report changed. No API, DTO, field, persisted format or executable
code changed; no tests or static checks were rerun. Prior focused validation
remains recorded in [release-pr-mutation-ownership.md](release-pr-mutation-ownership.md).
Remaining child ownership, historical telemetry linkage and bounded retry,
dated repair ownership, closed-obligation continuation and configured acceptance
stay open. The observed scorecard blocker, required UI/public proof and two
adjacent automatic settlements remain unresolved. No publication, landing or
Task completion follows from this compression review.

## Git ownership compression review at `fa0c6bbbb9`

No executable reduction selected. Before and after this pass: an obligation
retains original due opportunities; each execution owner retains attempts with
selection, verification and product outcome. Collapsed opportunities reference
that owner. `settle` writes outcome and verification atomically. Physical receipts
retain wrapper results; publisher receipts retain artifact stages and public
observations. Cron attribution, target exclusion and checkout protection remain
separate capabilities. Git inheritance extends their lifetimes without another
execution object or writer.

Inspected accounting types and `begin`, `finish_process`, `select`, `settle`,
receipt validation and history qualification; followed release preparation into
`commit_workflow`, stage/commit, upstream selection, ordinary and force-with-lease
pushes, and the preceding Task settlement fence. Checked the target/checkout
owners and adjacent PR finalization/compensation paths. The complete Task patch
was available without truncation (689,825 characters); this review concentrated
on those model paths and the latest Git inheritance cut.

Mirrors checked: CLI/Flow cron context, release-history text/JSON and disposition,
the history fixture and round-trip test, ordinary CLI/Flow/PM/Task/PR commit
callers, Python artifact/public receipt types and descriptor forwarding, the Git
survival fixture, release README, cron host guide and release-evidence direction.
Search found no Swift consumer of `ReleaseHistory`, `ReleaseOpportunity` or
`PublicReleaseReceipt`.

| Candidate | Why retained |
|---|---|
| Git output helpers with and without inheritance | They share one command implementation and one stdout/error conversion. Removing the private convenience entry points would spread no-op callbacks into unrelated reads; it removes no domain concept or competing implementation. No public legacy alias was introduced. |
| `inherit_git` and `inherit_pr` | These name actual, incomplete launch coverage in different operations. A combined execution context would add vocabulary and obscure the uncovered Task revocation/notes/tool paths. |
| Upstream establishment and ordinary push | One creates tracking for an explicit remote/branch; the other uses existing tracking and permits the existing force-with-lease fallback. A mode envelope would re-express those choices without deleting behavior. |
| Public push fence and private already-locked push | Task restart already holds the mutation guard. Collapsing the entry points would require reacquisition or weaken the fence before exposing a changed head. |
| Clean-worktree and committed-worktree push branches | Both use the same push owner. Flattening their control flow is local syntax cleanup, not a model/API reduction; a dirty worktree with nothing staged intentionally returns without pushing. |
| Target lock, checkout lease and cron descriptor | Their scopes remain target mutation, exact checkout removal and scheduled attribution. Merging them changes independent-scope behavior and authority. |
| Selection/outcome, process/product and stage/public evidence | Each pair records distinct facts at different boundaries. Removing selection loses pre-success recovery; combining process and product truth or stage and public proof overstates success. |
| Frozen coverage and current collapse links | Historical timing/provenance uses saved execution coverage; current links identify the owning result. Neither reconstructs the other after retries. |

Searches found no restored duplicate success-proof wrappers, separate
`record_verification` writer or Python candidate/publish receipt classes.
Historical schema-1 physical receipts remain required evidence. No API, DTO,
field, storage format or executable code changed; only this report changed.
No tests or static checks were rerun. The preceding focused passes remain the
evidence recorded in [release-git-ownership.md](release-git-ownership.md).
Task compensation, notes/tools/worktree child ownership, historical telemetry
linkage and bounded retry, dated repair ownership, closed obligations and
configured acceptance remain open. This compression review does not approve the
branch or establish either required automatic settlement.

## Lockfile ownership compression review at `e70332dd4c`

No executable reduction selected. Before and after this pass, an obligation
retains original due opportunities; each execution owner retains attempts with
selection, verification and product outcome. Collapsed opportunities reference
that owner. `settle` writes verification and outcome atomically. Physical cron
receipts describe process results; publisher receipts retain artifact stages and
public observations. Scheduled attribution, target exclusion and exact checkout
protection remain separate capabilities.

Inspected accounting types, `begin`, `finish_process` and `settle`, release entry
and history qualification, then followed preparation into the shared manifest
updater and its Cargo/uv commands. Checked standalone bump, target/checkout
inheritance and shared command output/error conversion. The complete Task patch
was available without truncation (719,646 characters); this review concentrated
on those model paths and the latest lockfile ownership change.

Mirrors checked: CLI/Flow cron context, release-history text/JSON, its JSON
fixture and round-trip consumer, Python artifact/public receipt types and
descriptor forwarding, the lockfile survival fixture, release README, cron host
guide and release-evidence direction. Search found no Swift consumer of
`ReleaseHistory`, `ReleaseOpportunity` or `PublicReleaseReceipt`.

| Candidate | Why retained |
|---|---|
| Standalone bump and release preparation | Both use one manifest updater. Preparation supplies capabilities it already owns; standalone bump supplies none. Combining their entry points would change user-visible behavior or introduce implicit ownership. |
| `inherit_tools`, Git and PR inheritance callbacks | These configure existing commands at distinct, partially covered boundaries. A generic execution context adds vocabulary without removing an owner or protecting the remaining notes/Task/worktree children. |
| Cargo/Python changed flags | They record whether any corresponding manifest changed, so each existing lockfile is updated once. A tool registry or manifest-state enum would replace two local facts with more machinery. The command loop is already shared. |
| `run_stdout`, `run_locked_stdout`, `command_stdout` | Construction differs, but output/error conversion has one implementation. Flattening the short constructors spreads syntax without reducing the domain or public API. |
| Target lock, checkout lease and cron descriptor | They protect different scopes: release mutation, exact checkout removal and scheduled attribution. Merging them changes authority and independent-scope behavior. |
| Selection/outcome, process/product and stage/public evidence | Selection survives before success; wrapper exit can disagree with settlement; public read-back proves more than completed publisher stages. Removing these distinctions loses recovery or proof. |
| Frozen coverage and current collapse links | Saved coverage preserves historical timing; current links identify catch-up ownership. One cannot reconstruct the other across retries. |

Searches found no restored duplicate success-proof wrappers, separate
`record_verification` writer or Python candidate/publish receipt classes.
Historical schema-1 physical receipts remain required evidence. No API, DTO,
field, storage format or executable code changed; only this report changed.
No tests or static checks were rerun. Prior focused validation remains recorded
in [release-lockfile-ownership.md](release-lockfile-ownership.md).

Task compensation, notes and source/worktree child ownership, historical
telemetry linkage and bounded retry, dated repair ownership, closed-obligation
continuation and configured acceptance remain open. The scorecard blocker,
required UI/public proof and two adjacent automatic settlements remain unresolved.
This compression review does not approve the branch or complete the Task.

## Notes ownership compression review at `d6e4504e78`

No executable reduction selected. Before and after this pass: an obligation owns
original due opportunities; each execution owner retains attempts with selection,
verification and outcome. Collapsed opportunities reference that owner. `settle`
writes verification and outcome atomically. Physical receipts describe process
results; publisher receipts retain artifact stages and public observations.
Scheduled attribution, release-target exclusion and checkout protection remain
separate capabilities. Notes input is a retained runtime prompt, not another
execution record or settlement writer.

Inspected accounting types, `begin`, `finish_process`, `settle`, receipt validation,
release entry and history qualification. Followed preparation and standalone notes
through `run_release_notes_stage`, bounded context construction, provider-error
fallback, prior-note restoration, runtime prompt persistence, nested CLI dispatch
and Codex harness launch. Checked target/checkout inheritance and cleanup. The
complete Task patch was available without truncation (749,284 characters); this
pass concentrated on these model paths and the latest notes ownership change.

Mirrors checked: CLI/Flow cron context, release-history text/JSON and disposition,
the history JSON fixture, Python artifact/public receipt types, notes input JSON,
the builtin release-notes skill, provider survival and notes-policy fixtures,
release README, cron host guide and release-evidence direction. Searches found no
Swift consumer of `ReleaseHistory`, `ReleaseOpportunity`, `PublicReleaseReceipt`
or `ReleaseNotesContext`.

| Candidate | Why retained |
|---|---|
| `ReleaseNotesContext` and selected release changes | Context is the bounded agent input, including decisions, previous voice and explicit omissions. Selection retains exact recovery identity before success. Merging them would either lose bounded-input semantics or burden recovery with notes data. |
| Typed context and serialized bytes | The builder measures the actual encoded size while trimming inputs; the same typed result feeds deterministic fallback. This is one value plus its encoding, with no independent writer or lifecycle. Changing the tuple to a wrapper or reserializing later removes no domain concept. |
| Previous-note backup and previous notes in context | The backup preserves the complete original file for failure restoration. Context contains a bounded excerpt for authorship. Deriving restoration from context could destroy omitted bytes. |
| Source limits and omissions | Limits state the enforced budget; omissions state what this invocation excluded. The skill consumes both and the tests preserve missingness. Removing either changes the agent's evidence contract. |
| Notes context file and assembled prompt file | The prompt supplies skill instructions; its referenced context supplies exact bounded release facts. Both use the existing runtime prompt writer. Removing the context path changes the skill's input contract and cannot be treated as a storage deduplication. |
| Unique context filename | It prevents a retry from replacing input still used by a surviving provider. It introduces no lookup service, execution identity or status. A timestamp-only filename would weaken preservation. |
| Notes/Git/PR/tool inheritance callbacks | They configure existing commands with borrowed capabilities. A combined context or executor would add machinery while Task compensation and source/worktree children remain uncovered. |
| Target lock, checkout lease and cron descriptor | Their scopes are release mutation, exact checkout removal and scheduled attribution. Combining them changes authority and independent-scope behavior. |
| Selection/outcome, process/product and stage/public evidence | Each pair records different facts and timing. Collapsing them loses recovery evidence or overstates success. Frozen coverage likewise preserves history that current collapse links cannot reconstruct. |

The notes slice already removed temporary-input cleanup as the input lifetime
owner and reused runtime prompt persistence. No alternate notes writer, provider
launcher or context store remains to collapse. Searches found no restored duplicate
success-proof wrappers, `record_verification` writer or separate Python candidate
and publish receipt classes. Historical schema-1 physical receipts remain required
evidence. Direct release worktree creation still disables default-branch sync;
this scoped inspection does not establish inheritance through every shared helper.

Only this report changed. No APIs, DTOs, fields, formats or executable code changed;
no tests or static checks were rerun. Prior focused results remain recorded in
[release-notes-ownership.md](release-notes-ownership.md). Task compensation,
source/worktree mutation children, original telemetry linkage and bounded recovery,
dated repair ownership, closed-obligation continuation and configured acceptance
remain open. The scorecard blocker, required UI/public proof and two adjacent
automatic settlements remain unresolved. This review does not approve the branch
or complete the Task.

## Source ownership compression review at `f8380883e2`

Removed the named-worktree API's obsolete main-sync mode. Before this pass,
`create_named_worktree` could both materialize a selected source and reset the
caller's default branch through `sync_main`. Both production call sites passed
`false`; only a test enabled the mode. After this pass, the helper only creates
the local checkout. Its caller owns source fetching/selection and explicit branch
publication. Ordinary placement retains its separate existing synchronization.
This removes an alternate source-mutation owner, rather than keeping preservation
dependent on every future caller remembering a boolean.

Deleted `sync_default_base`, its best-effort `sync_main` branch and import, and
migrated both production callers and all direct test callers. Replaced the test
that expected main to reset with a real-Git preservation case: origin advances,
the caller has an unpublished commit and staged/unstaged/untracked work, and
explicit-source and default-local-source creation both preserve caller HEAD,
branch, raw index and working bytes. The helper's documentation now describes
source ownership instead of calling it a compatibility helper. No compatibility
overload remains. No CLI command, DTO, persisted field or receipt format changed.

The release model remains an obligation containing original due opportunities;
each execution owner retains attempts with selection, verification and outcome.
Collapsed opportunities reference that owner. Atomic `settle` owns product
evidence; physical receipts describe process results; publisher receipts retain
artifact stages and fresh public observations. Cron attribution, target mutation
exclusion and exact checkout removal remain distinct capabilities.

Inspected the accounting types and settlement/receipt validation, history
qualification, release preparation and exact-source materialization, cleanup,
shared Git creation/removal and ordinary placement. The complete Task patch was
available without truncation (800,847 characters). Checked CLI/Flow cron context,
history and disposition output, the history JSON fixture, Python artifact/public
receipts and descriptor forwarding, direct helper callers, source survival tests,
release README, cron host guide and release-evidence direction. Searches found no
Swift consumer of the release-history/opportunity/public-receipt types.

| Candidate | Disposition |
|---|---|
| Named creation's main-sync mode | Removed across implementation and callers. Production already selected/fetched source explicitly; the only enabling caller was the obsolete reset test. |
| Ordinary Git entry points and inheriting variants | Retained. They delegate to one implementation; one offers ordinary public use, the other supplies already-held capabilities. Removing the wrappers would spread callback plumbing without reducing a domain owner. |
| Ordinary and owned removal | Retained. Independent acquisition protects against surviving children; borrowed removal uses an exact lease already acquired by its caller. The unchecked alternate implementation was already removed by the preceding slice. |
| Named source creation and ordinary placement | Retained. Release owns exact source and explicit publication; ordinary placement owns its existing branch synchronization. Unifying them would reintroduce implicit publication or mode switches. |
| Target lock, checkout lease and cron descriptor | Retained for their separate mutation, removal and attribution scopes. Combining them would change authority and independent-scope behavior. |
| Selection/outcome, process/product and stage/public proof | Retained because selection survives before success, wrapper exit may disagree with settlement, and public observations prove more than completed publisher stages. |
| Frozen coverage and current collapse links | Retained. Historical timing and intervention evidence cannot be reconstructed from current ownership after retries. |

The simulated review checked that the reduction removes the helper's reset
branch entirely, while keeping explicit release fetches, the exact-source
classifier, child inheritance and independent cleanup acquisition. It adds no
new guard, executor or ownership record. Searches found no restored duplicate
success-proof wrappers, separate verification writer or Python candidate/publish
receipt classes. Historical schema-1 physical receipts remain required evidence.

Validation: the focused `worktree_tests`
`create_named_worktree_preserves_caller_and_uses_selected_source` case passed
(0.62s execution). `cargo fmt --check` and
`cargo clippy --all-targets -- -D warnings` passed; Clippy took 26.36s including
build-lock wait. `git diff --check` passed.
The preceding twelve source-survival cases remain historical evidence; their
inheritance paths did not change and were not rerun for this API reduction.

Task compensation, original telemetry linkage and bounded recovery, dated repair
ownership, closed-obligation continuation and configured acceptance remain open.
The scorecard blocker, required UI/public proof and two adjacent automatic
settlements remain unresolved. No installation, cron trigger, production release,
PM handoff, PR publication, landing or Task completion is part of this pass.
