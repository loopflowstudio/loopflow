# Implementation choices — LOO-412, 2026-10-08

Jack Heart's custom-ref steers supersede host callbacks. The
[current design](work-on-another-machine-name.md) records the accepted boundary,
deletion cut and remaining acceptance. Earlier callback assumptions are retained
at `b040c7c8d:scratch/questions.md`.

- LOO-406's common writer is integrated from main `d20c56daf` at `225b8c95f`. Peer
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
  failures still roll back the import. Association storage is implemented; public Git/HTTPS composition remains.
  Conflicts preserve both IDs and their execution histories.

The cut remains unpublished. The design owns remaining integration and acceptance;
earlier adoption and pre-cut storage evidence, with their limitations, remain at
`79e478550:scratch/questions.md`, the paragraph before selection choice.

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
- Setup choices remain in the design and at
  `6f1869e9b:scratch/questions.md`, October 8 setup choice. A stable user key,
  empty local connection and future-root routing keep joining separate from
  publication. `use local` never withdraws existing membership; descendants
  inherit their parent's saved selection. Mixed-provider exchange now uses the common owner; combined acceptance remains.

- Provider-frontier representation and acquisition-before-projection rationale:
  `b4a91632e:scratch/questions.md`, October 8 provenance. The existing journal
  retains raw provider bodies, revisions and original ages; only matching values
  receive Linear priority. Common acquisition, independent evidence and savepoints
  remain the owners. Storage proof is not mixed-provider acceptance.

- Foreground/short-command ownership and October 8 review findings remain at
  `bf4c39b2e:scratch/questions.md`, the foreground and short-command entries.
  Saves commit first; bounded worker-owned attempts retain locks through cancellation.
  Active connections supply reconnect; setup never publishes or retries a turn.

- October 8 conflict, disposition, comment, status and semantic-content choices
  are consolidated in the design's implemented boundaries. Detailed rationale and
  counterexamples: `8f3472eda:scratch/questions.md`, the five October 8 entries.
  Valid contradictions isolate projection; malformed input aborts import. Retain
  losing receipts, provider authorship, destination-scoped unknown status and the
  common content parser. No mixed-provider activation follows.

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

- Effect-eligibility and deletion representation choices are consolidated in the
  design's implemented boundaries; full rationale remains at
  `de3c84b08:scratch/questions.md`, the October 9 effect-eligibility and
  foreground/deletion entries. Holds defer effects, never saves/acquisition;
  common receipts retain original times, baselines, uncertainty and explicit
  acknowledgements. No new product policy is selected.

- Ordering choices and counterexamples live in the design's
  **Implemented boundaries, not remaining implementation**, Ordering; detailed
  rationale stays at `fcd64901f:scratch/questions.md`.

- October 9 alternate-acquisition, causal invalidation, unplaced-Wave and scoped
  Desktop representation choices remain in the design's implemented boundaries;
  full rationale: `e08dc9312:scratch/questions.md`, the four entries after Ordering.
  Independent negative facts, original ages, unknown placement and last-good scoped
  readings remain required; none enables mixed-provider exchange.

- Association's refusal-only findings remain at
  `90a37ab79:scratch/questions.md`, October 9 association finding. The design now
  owns joint projection and receipt composition; private/projection holds remain.

- October 9 correspondence choice: explicit local `planning associate <incoming-id>
  --with <local-id> --linear <provider-id>` uses retained scalar mapping evidence,
  not names or creation inputs. Exact local rows retain execution ownership; an
  absent incoming full ID can resolve to that owner. Correspondence never enrolls
  either side or clears projection/effect conflicts. Ordinary import now retains
  each associated creation receipt under its original ID on the local owner;
  no physical identity or captured input is reassigned. Common acquisition owns
  exact readback. The joint cut below supersedes its scalar projection hold. Lookup rechecks
  local mapping/repository and contradictory incoming scalar claims. This is a
  reversible composition choice, not new policy. Tests now enter through ordinary
  association/import/acquisition; the design records the later focused pass.

- October 9 causal representation: retain the existing journal rather than export
  machine-local correspondence. Common acquisition may capture a cross-origin parent
  only for the same kind, field, value and exact Linear body under an explicit
  association. The provider fact, not identical text or the association row, is
  portable evidence. Heads remain per-origin; delivery baselines traverse the links.
  Dependency closure is validation-only: unselected origins and dependents remain
  held, never enrolled. Review caught a preserved local value incorrectly borrowing
  provider parents; both scalar and content capture now require accepted equality.
  This was a partial implementation choice, not new policy from Jack Heart.
  The joint-owner cut below supersedes its Linear-only causal-link validation;
  unobserved retained provider heads still require accepted equality.

- October 9 joint-owner and membership choices are consolidated in the design,
  **Remaining integration**, item 3. `1cbbba625`/`01769ea0d` reuse accepted sources
  for causal scalar winners; `79e478550`/`521c7040f` resolve membership delivery
  and readback without rewriting captured values. These are reversible composition
  choices, not new product policy. Private groups remain held; selected associated origins now exchange; order/deletion origins derive from the journal's unique receipt
  identity, not a second mutable column. The validator rejects cross-origin reuse.

- Recovery-presentation choice and proof are consolidated in the design's
  implemented boundaries; full rationale: `f55d6c5b7:scratch/questions.md`, final
  entry. No restoration writer, author inference or implicit enrollment.
- October 9 public Git/HTTPS composition exposed duplicate field delivery after
  a lost reply: an unattempted equal-desired receipt survived provider readback.
  The common writer now adopts a changed Linear baseline without acknowledging
  an unattempted effect; unchanged baselines still preserve saves. This replaces
  the old membership-test assertion, not Jack Heart's Linear-wins policy. Public
  creation/link origins and broader combined acceptance remain distinct.
