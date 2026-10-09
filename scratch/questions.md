# Implementation choices — LOO-412, 2026-10-08

Jack Heart's custom-ref steers supersede host callbacks. The
[current design](work-on-another-machine-name.md) records the accepted boundary,
deletion cut and remaining acceptance. Earlier callback assumptions are retained
at `b040c7c8d:scratch/questions.md`.

- LOO-406's common writer is integrated through pinned `efc90abb1` at `7b5a12e2b`. Peer
  import/export, ordering and local-only foreground composition exist in source;
  execution verification remains, independent of the parent's Linear delivery.
- Jack Heart's newer policy: Linear wins observed conflicts; otherwise host
  preference where appropriate, then last-write-wins with retained losing edits.
  Use user-keyed destination selection by default, explicit opt-in shared planning.
  Exact stable key setup and deterministic ties are implementation choices here.
- Planning remote selection must be explicit before ordinary commands can publish;
  no real planning data goes to the public code remote. Current transport tests
  use synthetic documents and disposable local remotes.
- Implementation choice: causality precedes a hybrid logical clock and stable
  change-ID tie-break. No initiating host has blanket preference over collaborators;
  positive Linear-origin observations win concurrent peer writes. Preserve losing
  mutations. Public setup now provisions/recovers an explicit user key and binds an empty
  destination. Local-only foreground exchange and Desktop destination status are connected; mixed-provider composition remains unfinished.
- `new_migration.py` identified the inherited LOO-406 draft as this Task's draft.
  Git's explicit stack boundary establishes that it belongs to the parent;
  LOO-412 therefore has one separate `planning_peers.sql` draft depending on it.
- October 8 implementation choice: project each object under a savepoint; retain
  expected constraint failures and their mutations while independent objects commit.
  Wave selection follows Project projection to break their reference cycle. Invalid
  documents, reused mutation IDs, foreign repository ownership and unexpected SQL
  failures still roll back the import. Legacy-provider association remains unfinished;
  conflicts preserve both IDs rather than guessing which execution history to use.

The integrated cut remains unpublished. The current design owns verification and
remaining integration; earlier adoption evidence stays at `5d336868f:scratch/questions.md`.
October 9's 63 peer-storage passes at `25cbc0a09` cover alternate acquisition,
not the later invalidation cut. `6ccbbabe3` records four focused passes for that
cut; earlier 49-test evidence remains at `b21429d9b`. The design separates these
storage proofs from public composition and remaining implementation.
Broader public/DTO, provider-lifetime, cold-worker, Desktop and mixed-provider acceptance
remain separate. No new product decision is selected.

- October 8 selection choice: selecting a Wave includes its descendants; joining
  selects nothing. Existing moves never enroll a private parent or change the
  record's saved destination. The move-after-join counterexample is retained at
  `a43f3e2ea:wave/infrastructure/MEMORY.md`, “Tasks across machines.” Repair:
  `b5dafd918`; selection simplification: `7395cf3bb`.
- Reversible implementation choice: hold a record's entire journal and dependents
  out of exchange when any retained reference is unselected. The implementation
  retains accepted local/provider moves and every mutation, omits rather than
  deletes the peer's last shared state, and skips held-record projection while
  independent imports commit. The Store conflict reader derives this conflict
  from the same journal/membership snapshot. Moving back cannot release private historical references; only explicitly selecting
  the referenced work can. This conservative boundary is not a convergence claim
  or Jack Heart's approval of a new UX. CLI holds are displayed; recovery/losing-edit
  UX still needs review. Repository-scoped membership also prevents a shared endpoint
  from transferring another repository's identities.
- October 8 setup choice: `planning key --new` explicitly creates one stable user
  key; repeated calls retain it, and `--recover` refuses to replace a different key.
  `connect` binds an empty destination without activating it. `use` routes future
  root Waves only (including newly ingested definitions); `use local` does not
  withdraw existing membership. Children follow their parent's saved selection.
  This keeps joining separate from publication consent. All setup is local;
  grouped Linear receipts now exist; legacy association and mixed-provider
  activation remain unfinished. Local-only exchange uses the foreground lifetime.

- Provider-frontier representation and acquisition-before-projection rationale:
  `b4a91632e:scratch/questions.md`, October 8 provenance. The existing journal
  retains raw provider bodies, revisions and original ages; only matching values
  receive Linear priority. Common acquisition, independent evidence and savepoints
  remain the owners. Storage proof is not mixed-provider acceptance.

- October 8 foreground safety choice: until all provider paths are composed and
  verified, Linear-connected repositories retain plans and report the gap instead
  of activating unsafe peer exchange. This is an internal incomplete boundary, not
  Jack Heart's final product policy. Publication always fetches/imports first and
  persists uncertainty before pushing; independent acquisition bypasses its effect
  lock. Current eligible-export digests distinguish later saves from confirmation
  of an earlier revision. Provider launch now owns the lifetime independently of Task attribution;
  short Task saves and Project edits attempt exchange after committing. Desktop
  destination status is implemented; per-Work sharing/recovery presentation remains. Review caught two issues repaired inline:
  cancellation/status writes must retain the worker's lock, and target-only machine
  dispatch must not require a repository on the initiating machine.

- October 8 short-command choice: save locally first, release planning locks, then
  await one bounded Git publication attempt (including acquisition). Failure reports
  pending sync without reversing the save. No detached worker or command retry is
  added. An active provider/work-watch connection handles later reconnect; with no
  connection, the next mutation or foreground connection supplies another attempt.
  Explicit setup still performs no exchange. Existing branch selectors survive
  lifecycle lookup; the foreground loop observes destinations added after launch.

- October 8 conflict classification: valid contradictory/unordered Task/Project
  provider observations use `ProviderObservationConflict`, not malformed-input
  `InvalidData`. Peer imports retain the existing accepted frontier and the entire
  mutation union while isolating projection; direct provider acquisition still
  rejects. Malformed documents and unexpected SQL remain transaction-wide errors.
  Review strengthened the regressions to assert attempted/acknowledged/conflict/error
  receipt state, not only the pending value reader, which omits those fields.

- October 8 disposition cut reuses the common state receipt/reconciliation owner,
  preserving causal baselines and uncertain attempts without inventing a local
  Workflow move. Review caught unchanged-baseline preservation incorrectly retaining
  a losing intention after a peer Linear winner; peer-winner reconciliation now
  explicitly retires it. Cached non-lifecycle states still project without minting
  unsupported lifecycle deliveries; the common writer still owns its three targets.
  No new product policy is selected. Grouped receipts now exist; composed checks
  stay in the design. Mixed-provider exchange is still disabled and not ready
  for publication.

- October 8 comment composition uses the existing comment ID as delivery identity,
  raw provider observations as journal provenance, and the common thread/receipt
  owners. Acquired migration rows without a raw observation stay local until read;
  they cannot be exported as authored comments. Local append remains immutable:
  conflicting reuse of a comment ID isolates projection and preserves its journal,
  rather than inventing a comment-edit delivery protocol. These are reversible
  representation choices, not new product policy. Review removed incomplete-record
  export and corrected grouped JSON ordering; public composed acceptance remains.

- October 8 status choice: pending local changes become unknown when a destination's
  journal cannot be read. Conflicts belong inside that destination's status, read in
  the same transaction. Retained receipts/conflicts survive; unavailable sharing holds
  are not reported as empty convergence. Only malformed journal errors are isolated;
  database failures still fail status. This replaces the second global conflict reader,
  not the journal or its validation. `de9470d5a` also shares the destination-scoped
  conflict query with import settlement. Storage checks passed; public/DTO execution
  remains with gate.

- October 8 semantic-content choice: retain existing common Markdown storage,
  but remove it from peer ordering. Rust's content owner captures workflow, the
  KR list and the target list in the existing journal; each list keeps its common
  receipt granularity. All production creation/acquisition/content writers capture
  before committing. The existing migration data-hook pattern seeds populated
  content with that parser in the same transaction, rather than adding a SQL
  parser, a second table or export-time edits. Malformed retained content fails
  migration without discarding its bytes. This is an implementation choice, not
  new policy from Jack Heart. Review removed an incorrect test expectation that
  an observed concurrent Linear winner must preserve a losing local field; the
  selected winner retires its intention and retains the losing receipt. Independent
  edits without a competing observation remain covered by the passing storage suite.

- October 9 executable feedback exposed two missing common-writer boundaries:
  accepted peer provider facts lacked their original acquisition age on local rows,
  and readiness treated missing list inventory as negative membership evidence.
  Import now retains the accepted age; readiness still rejects explicit archive,
  unresolved membership and configured-Initiative contradictions, not missing lists.
  Imported planning remains unplaced. A later local placement operation uses the
  local machine when its parent is unplaced and preserves existing placements.
  Task creation readback uses the original common receipt even if a peer mapping arrived
  first; mapping alone neither acknowledges nor clears uncertainty. Review rejected
  freezing Project membership to its creation snapshot: later accepted moves must
  remain possible. Project/link readback therefore stays with its existing
  owner pending full receipt composition. The later receipt-transport cut below
  supersedes the missing-transport finding;
  foreground creation has focused proof. Ordering and removal/archive/Team transport
  now exist; public invalidation verification and mixed-provider activation remain
  unfinished.
  No new product policy is selected.

- October 9 effect-eligibility choice: a rejected or sharing-held peer object
  cannot authorize a new common field/deletion, creation/link, state or ordering
  effect from its incomplete local receipts. Check existing conflict receipts,
  including skipped imports, inside the attempt transaction without blocking saves
  or acquisition. No new effect queue or automatic retry is introduced. A newer
  acquisition cannot erase the conflict; successful import must incorporate the
  retained attempts first. This conservative object-level deferral is an internal
  composition choice, not a new product policy. Mixed-provider exchange remains
  disabled. Creation-column ownership rationale remains at
  `cdec83fe8:scratch/questions.md`, October 9 receipt transport, and in the design.

- October 9 foreground/deletion choices are implemented, not new product policy:
  one common export view exposes unprepared plans and explicit acknowledgement;
  acquisition never places work. Optional `deletion:<receipt-id>` journal fields
  transport common receipts with original save times, baselines and monotonic
  attempts/acknowledgements. Common visibility reconciliation replaces sequence
  selection. Unordered active/trash evidence isolates projection; no inferred
  deletion settlement. Full rationale and prior checks:
  `225b8c95f:scratch/questions.md`, final two entries. The current design owns
  remaining public invalidation verification and presentation work;
  mixed-provider exchange remains disabled.

- Ordering choices and counterexamples live in the design's **Remaining integration**,
  item 2; detailed rationale stays at `fcd64901f:scratch/questions.md`.

- October 9 alternate-acquisition choice: common removal/archive evidence commits
  before scalar projection, so rejecting a stale body cannot erase the negative
  fact. Team confirmation keeps its causal heads and prior Team set; concurrent
  contradictory confirmations or unrelated retained membership block projection,
  never choose by entity revision or receiving time. Legacy negative evidence has
  unknown acquisition age; no Team evidence is fabricated by migration. This is
  an implementation choice, not a new product decision. Mixed-provider exchange
  remains disabled; full-list acquisition is still an independent boundary.
  The later causal invalidation cut below supersedes omission of non-removal
  notifications; public composition remains unproved before mixed activation.
  Unknown legacy ages never
  rewrite a Task's removal timestamp. Each independent fact gets its own savepoint,
  so Team contradictions cannot roll back an accepted archive.

- October 9 replay-safe invalidation choice: use causal heads in one optional
  `provider_invalidation` journal field. Notifications and successful detail
  readbacks share that field; a detail retires only notifications it observed.
  Concurrent/unseen notifications remain invalid, irrespective of receiving time.
  Acknowledgements carry identity/revision/age only and clear freshness after an
  accepted matching-or-newer frontier; no duplicate body or parent mapping travels.
  List/scalar replay cannot clear freshness. Null-detail invalidation uses the same
  capture path, and migration retains unknown-age invalid caches. No new provider
  revision, execution authority or mixed-provider activation is introduced.

- October 9 unplaced-Wave presentation uses the existing optional placement query;
  execution's required reader stays strict. Null means unplaced, never local by
  default. Rust/Swift share the nullable DTO; no migration or new authority is added.

- October 9 Desktop status reuses the common destination reader in a scoped Work
  frame. Read failures retain last-good evidence; scope/Machine changes fence old
  replies. No status cache, sync worker, setup action or mixed activation is added.
  Per-Work selection and losing-edit recovery remain unfinished.

- October 9 association finding: the earlier duplicate-mapping fixture cleared
  `external_issue_id` directly. The common export view makes that legacy Task a
  creation candidate while retaining its old issue selector and effect receipts.
  This is not safe association. Preserve the existing mapping against ordinary
  peer replacement/removal, and retain the rejected journal. An explicit
  planning-only correspondence must resolve local lookup and effect ownership
  without renumbering either Work, exporting private history or merging execution.
  Representation remains unfinished; no product decision or transfer approval is
  attributed to Jack Heart. The design's item 3 replaces the raw-SQL recovery path.
