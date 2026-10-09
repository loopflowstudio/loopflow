# Implementation choices — LOO-412, 2026-10-08

Jack Heart's custom-ref steers supersede host callbacks. The
[current design](work-on-another-machine-name.md) records the accepted boundary,
deletion cut and remaining acceptance. Earlier callback assumptions are retained
at `b040c7c8d:scratch/questions.md`.

- LOO-406's common writer is integrated through pinned `ffe986160`. Peer
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
  destination. Local-only foreground exchange is now connected; mixed-provider composition remains unfinished.
- `new_migration.py` identified the inherited LOO-406 draft as this Task's draft.
  Git's explicit stack boundary establishes that it belongs to the parent;
  LOO-412 therefore has one separate `planning_peers.sql` draft depending on it.
- October 8 implementation choice: project each object under a savepoint; retain
  expected constraint failures and their mutations while independent objects commit.
  Wave selection follows Project projection to break their reference cycle. Invalid
  documents, reused mutation IDs, foreign repository ownership and unexpected SQL
  failures still roll back the import. Legacy-provider association remains unfinished;
  conflicts preserve both IDs rather than guessing which execution history to use.

Peer source is committed through `de9470d5a`; this integrated cut remains unpublished. Current verification and
remaining integration are in the existing design; previous adoption-only evidence
is retained at `5d336868f:scratch/questions.md`.

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
  grouped Linear receipts and legacy association remain unfinished; local-only
  exchange now uses the existing foreground lifetime.

- October 8 provenance implementation: replace the origin-only boolean with the
  accepted provider body, identity/revision and original acquisition time in the
  same mutation journal. Reuse existing provider acquisition tables/checks on import;
  do not add another frontier store. One field map governs capture and validation.
  Only values matching the actual provider fact gain Linear priority; preserved
  local intentions retain local origin. Equal-value acquisitions capture missing
  provider frontiers, including migrated baselines. Causal receipt baselines retain
  provider revisions, not the receiving machine's clock. This reversible representation
  duplicates the immutable observation across field receipts; the existing document
  size bound still applies. It is not a new product decision from Jack Heart.
- `e608b0d2d` supplies the requested provider-frontier representation; its regressions
  are authored, not executed. October 8 repairs move acquisition before projection
  and reassert typed membership-conflict evidence after the object's rollback.
  Identical bodies still undergo removal/archive checks; superseded entity revision
  groups remain journal history, not reacquisition. Common no-op upserts preserve
  readback idempotence. SQL checks pass; Rust regressions are unexecuted. Grouped
  receipts, alternate provider paths and independent relationship ordering remain,
  alongside legacy association and Desktop presentation. No mixed-provider
  completion or compare-and-swap is claimed.

- October 8 foreground safety choice: until grouped receipts and all provider paths are
  composed, Linear-connected repositories retain plans and report the gap instead
  of activating unsafe peer exchange. This is an internal incomplete boundary, not
  Jack Heart's final product policy. Publication always fetches/imports first and
  persists uncertainty before pushing; independent acquisition bypasses its effect
  lock. Current eligible-export digests distinguish later saves from confirmation
  of an earlier revision. Provider launch now owns the lifetime independently of Task attribution;
  short Task saves and Project edits attempt exchange after committing. Desktop
  peer-status presentation remains. Review caught two issues repaired inline:
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
  No new product policy is selected. Remaining grouped
  receipts and executable checks stay in the design; mixed-provider exchange is
  still disabled and not ready for publication.

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
  conflict query with import settlement. Public/DTO regressions remain unexecuted.

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
  edits without a competing observation remain covered. Rust tests are unexecuted.
