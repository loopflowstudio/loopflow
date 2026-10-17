# LOO-497 — Shared foundations for query-based views

## Authorized scope — October 16, 2026

Jack Heart requested the interactive query-view prototype, then authorized
technical foundations underlying any UX/API while unavailable for review.
Preserve `scratch/query-view/` for that review. Its grammar, parser, layout
heuristic and fixture data remain disposable design evidence. This foundations
PR does not complete LOO-497 or authorize shipping its remaining UX.

The complete pre-implementation plan is preserved at
`refs/loopflow/evidence/loo-497-foundations-design` (blob
`e737550f687c0d5f17c363e1ebe2c00a31f8fcde`). Broader context and later integration
acceptance remain in `desktop-api-coordination.md`, the brief and layout direction.

## Contract and ownership

- Existing Desktop inspection exposes actual outer geometry, retained inner
  trees, Work keys, leaf incarnations/surfaces, focus, zoom and hidden panes.
  `desktop state` replaces `list`. Reads acquire nothing. Outer hide currently
  drops its split position; state reports retained-but-unselected Work honestly.
- Rust `work::selection` owns typed repository/Wave defaults, explicit Work,
  ordered composition and saved-query references. Authored source IDs plus Work
  identity distinguish occurrences and survive sibling insertion. Missing
  references are unavailable evidence; cycles and invalid node IDs are errors.
- Defaults include started unfinished Tasks, including paused/waiting/idle;
  exclude backlog and finished Tasks. Explicit refs can select those too.
  Containers include immediate child Waves and Tasks throughout descendants;
  empty Waves remain. No subwave creation or UI is added.
- Arrangement retains a binary tree, axes/ratios, hidden members and auto/touched
  intent. Replacement requires every current occurrence exactly once. First
  visible Work occurrence wins in tree order; focus has no role. Hide/restore
  retains current edits and promotes the next duplicate without creating content.
- lfdb owns saved expressions and portable arrangements, atomically updated with
  revision checks. No derived-result cache, titles, native handles or liveness.
  Reads join existing Work readers and saved definitions inside a savepoint.
  Incomplete reads preserve earlier members/order; complete departures prune.
  Touched arrangements retain geometry and append arrivals deterministically.
- Swift WorkLayout/MultiplexerStore remain the live native owners. This slice
  projects their state; it does not add a second native arrangement engine.
  LOO-489 owns presentation/integration; LOO-416 owns provider/surface retention.
  LOO-489 now records a locally implemented shared pane renderer/projection;
  integration and native continuity remain unproved. No sibling files changed.

Jack authorized schema evolution. One draft, `saved_selections.sql`, grows in
place. Shared Work identity lives in `work::selection`; Desktop imports that owner
directly. Existing Task/Wave readers now share connection-level helpers, preserving
one membership source and composition with the Work observer's pinned snapshot.

## Delete — do not maintain

Foundations remove the `desktop list` spelling from dispatch, schemas, current
docs and tests, plus the Desktop compatibility re-export of shared Work identity;
preserve passive transport and exact-target I/O. No inspection cache or Swift
query evaluator was added. The broader list (split-as-shell,
leaf-dropping outer hide, Swift working-set predicates and selection mutations)
belongs to the later live-integration cut, not this bounded foundations PR.

## Remaining

The foundations diff awaits gate's complete native build and closed-app transport
acceptance; demo owns rendered judgment. Jack's October 16 pursue authorization
permits publication for review, not merge or Task completion. The live Workflow
records that same boundary. Full integration remains in
`desktop-api-coordination.md`, including prototype review, production queries,
navigation/commands, host plans, persistence transport and scripting parity.

## Reconciliation — October 16, 2026

Code inspection confirms three limits of this slice:

- `SelectionWork` carries membership facts only. Store reads order Waves by
  name/ID and Tasks by rank/ID; automatic trees follow that supplied order.
  Attention-first order and Task-before-Wave presentation remain accepted
  integration requirements, not properties of the foundations resolver.
- `saved_selections` stores repository/query-keyed intent locally. It does not
  bind each Work/default view to its saved query or exchange records across
  Machines. Native pane intent and app-owned file draft recovery are also absent.
  Portable fields alone do not establish cross-machine restoration.
- Shared resolution has store/library consumers only. Planning revisions advance
  on saves, but RegistryQuery and live Desktop do not yet consume selection
  results. The state DTO describes existing Swift geometry, not the future
  query composition or its duplicate suppression.

Upstream `463c5803a2` already supplies durable repository identity across directory
moves; the opening locator remains separate. No path conversion shim is needed.
The observed `origin/main` at `72dbe0344f` releases that migration as
`0.14.4.001_release.sql`; it adds no selection integration. After target integration,
gate's migration check must cover that released frontier with this Task's single
`saved_selections.sql` draft. Earlier migration evidence predates this release.

Fresh sibling notes confirm complementary foundations: LOO-489's
`foundation-result.md` records the shared `WorkspacePanesView` and
`TaskPaneProjection`, with five focused projection tests passing. LOO-416's
`loo-416-keep-task-sessions.md` records application-owned registry retention and
62 passing retention/destination tests, but no durable recovery or attach-only
transport. These are sibling checkout reports, not landed or combined acceptance.
Their persistence boundary expects LOO-497's shared schema/API to cover native
pane intent and file drafts; the current container-only schema is insufficient.
No second serializer or lifecycle owner is selected.

Product memory's older shell-allocating Split and exclusion of cross-launch
restoration describe prior scope, superseded by Jack's October 16 direction.
The selected Flow curates Product memory; no child memory directories exist here.
Grammar, sorting syntax, inbox appearance and pinning UX remain draft. Schema
permission, started-unfinished defaults and first-wins presentation are accepted.

Saves reuse one Work reading and definition map for every catalog entry. Selection
expansion validates each authored tree at its boundary instead of repeatedly
validating composition subtrees. A conditional DELETE owns revision checking in
one statement. Complete reconciliation uses its supplied order directly; partial
reconciliation unions retained and observed members without repeated scans.

Review finding: an incomplete auto refresh could reorder a retained duplicate
behind its later occurrence. Reconciliation now retains prior order before
appending new evidence, with a regression test. The released fixture records
Started history explicitly and supplies current Task fields; tests never weaken
production admission or replay the draft already loaded by `open_ephemeral`.

Prior checks and exact commands remain at
`refs/loopflow/evidence/loo-497-before-realign` (blob
`03aaaed7abb0f99a23770776f2bd3590c93964ee`): 10 focused Rust tests, two CLI
integration checks, 70 Swift tests plus two state/DTO checks and one durable-ID
check passed, as did lint and migration validation. No production code changed
during reconciliation. Publication review resolved the earlier broad selection
failure: the binary-selection fixture lacked the shared environment lock, allowing
other tests to change `LF_HOME` during dispatch assertions. The test now holds
that lock; all 34 broad selection tests pass. This establishes no native acceptance.

Check (October 16): `git diff --check` — passed; existing behavioral results reused for prose-only reconciliation; updated released-frontier validation deferred to gate after target integration.
