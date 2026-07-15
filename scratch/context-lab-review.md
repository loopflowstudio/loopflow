# Context Lab design review

## What changed structurally

Context Lab introduces one read path and one intervention path:

```text
agent launch capture
  → local trace/context ledger
  → Rust SessionSetQuery + atomic ContextLabSnapshot
  → native Swift flame / lanes / table / evidence
  → explicit lf trace body reader
  → existing Task workspace + refine skill
```

Rust owns population filters, trace joins, canonical source/revision identity,
token attribution, representative selection, and the flame hierarchy. Swift
owns view state, linked selection, saved queries, rendering, and the guarded
handoff. This reinforces the existing daemonless `RegistryQuery` boundary; it
does not introduce a second telemetry store, editor, agent host, or git path.

The design intent still matches `scratch/instruction-workbench.md`: research a
set of real sessions first, move from aggregate pressure to immutable evidence,
then start a separate refinement session against the canonical source.

## Key choices

- The session set is the primary object. Instruction rows are derived from its
  measured context rather than maintained as an admin catalog.
- Flame identity is `kind → canonical source → content hash`. Historical
  revisions remain immutable and naturally accumulate evidence.
- Prompt and conversation bodies remain closed until **Open trace**. The graph
  and evidence rail carry only measurements, hashes, availability, and exact
  addresses.
- Refinement refuses copied text and in-place historical mutation. It must map
  one current canonical source hash into an existing Task worktree.
- Provider-total-only turns remain in coverage denominators but contribute no
  flame or lane width. This keeps missing assembly attribution missing and makes
  flame widths reconcile exactly with measured supplied context.

## Live evidence

The rebased branch reader queried the production Loopflow ledger over 30 days
through the explicit development-binary opt-in. On July 15 the population
contained 54 sessions, 124 launches, and 132 turns. The largest aggregate band
was Wave memory; operating instructions were third. This contradicted the
starting hunch that `LOOPFLOW.md` would necessarily dominate and validated
ordering by observed supplied-token load.

After excluding eight provider-total-only turns from attributed geometry:

- supplied context: 973,485 tokens across 124 assembled turns;
- aggregate flame root: 973,485 tokens;
- sum of root children: 973,485 tokens;
- lanes with assets but no supplied total: 0.

The fresh canonical `LOOPFLOW.md` revision now resolves to
`/Users/jack/src/loopflow/rust/loopflow/src/engine/builtins/LOOPFLOW.md` with
captured and current effective hash
`130b91c3afb3afa7897e22cb85068a1714ab6431469dee3392eda10eb8bdd4fe`.
Five natural turns across four sessions contribute 5,825 tokens. One smooth
complete representative is
trace `0dd4ea7e-8b88-43d9-850a-014fcb631e79`, launch
`e614b88f-605d-4f6f-aa1d-c49cce779a57`, turn
`35ab6c81-21a2-4904-a60a-e1bda398a758`. An explicit body read resolved that
exact address to a 5,408-byte system prompt, 23,540-byte task prompt, and
25,657-byte normalized conversation; all three artifacts were present.

Natural revision grouping is also present without fixture events. For example,
`scratch/context-lab-review.md` appears as three hashes under one canonical
source: `b9e46420eb6c…` has one exposed launch, `19f30135a085…` has five, and
the newest `83f94dda9ea2…` has one. The latest-pair comparison correctly remains
unavailable because one population is below the three-launch minimum.

The high-context operating-guide representative at trace
`556f8b57-a406-4455-be8a-99b0acd2b60d`, launch
`e3f2b14f-054a-4c12-85a1-079344ff1293`, turn
`73d5bc08-ed47-4f77-aa78-054deda5c501` opens a 5.1 KB system prompt, 56.4 KB
task prompt, and 270 KB normalized conversation. The first body read occurs only
after the explicit trace action.

## Review changes already made

- Rebased through PR #906 so the development binary can read the current live
  ledger schema.
- Preserved the explicit development-to-production-ledger opt-in through the
  native app launcher.
- Resolved a Task worktree launch path to its canonical main-repo filter.
- Let `lf trace` accept the trace id carried by Context Lab as well as an exec id.
- Removed unmeasured provider-total-only assets from flames and lanes.
- Made missing canonical-source refinement visibly disabled and made trace body
  backgrounds explicit for readable native rendering.
- Added durable Project/Task launch attribution and corresponding Context Lab
  facets. Historical launches remain unattributed instead of being guessed from
  their worktree paths.
- Carried the selected Task Session identity into the fresh refinement process,
  so its trace and lifecycle commands stay attached to the chosen workspace.
- Re-read the roadmap before refinement side effects and reject a Task that is
  no longer idle or whose Wave, Task Session, repo, or worktree changed after
  selection. The handoff now opens the new Task terminal instead of an Agent tab
  that is necessarily inactive for eligible refinement Tasks; failed command
  dispatch removes the just-created terminal instead of leaving an orphan.
- Tightened the workspace identity check to include Project Session, Task,
  workspace slug, and branch as well as Wave, repo, Task Session, and normalized
  worktree path. A same-path branch replacement no longer passes as the selected
  workspace.
- Kept effective revision semantics in Rust. Evidence now carries the current
  source-file hash beside the effective prompt hash; Swift compares exact Task
  worktree bytes to that receipt instead of duplicating operating-guide wrapping
  and skill-frontmatter parsing.
- Corrected **Selected-source share** sorting. It had sorted raw selected tokens,
  so a 600-token slice in a 2,000-token launch outranked a 500-token slice in a
  1,000-token launch. It now sorts the measured ratios and uses raw selected load
  only as a tie-breaker.
- Corrected representative semantics: **Smooth complete** now requires complete
  capture, completion, and zero steering. A steered completion can still appear
  as high-context or failed/steered evidence, but not as the smooth baseline.
- Restored evidence parity by including the focused flame node in the table,
  showing the full selectable revision hash and first observation date, and
  keeping **Open trace** available even when prompt and conversation badges are
  missing so the exact address can reveal explicit artifact reasons.
- Removed a stale post-rebase Swift test for the deleted `RegistryQuery.backlog`
  API; roadmap is the single Task read and its existing fixture owns that proof.
- Proved the production migration chain through
  `0.11.006_context_launch_work`. The older isolated development database still
  carries a pre-rebase local `0.11.004_context_launch_work` stamp and is not the
  production-ledger demo surface.

## Independent review pass (2026-07-15)

A separate reviewer read commit `e71c8b5` end to end against the design's
"Done when" and found no correction or simplification owed. The pass verified,
did not just restate, the prior changes:

- **The source-hash split is a real reduction, not a rename.** Rust now returns
  both the effective prompt hash and the raw source-file hash from one read
  (`hash_current_source`), and Swift's launch guard compares exact Task-worktree
  bytes against `current_source_sha256`. This deleted `effectiveSourceHash` and
  `skillBody` from Swift, so operating-guide wrapping and skill-frontmatter
  parsing live in exactly one language. The safety chain still closes:
  `isEditable` proves the current file's *effective* hash equals the studied
  revision, and the byte guard proves the worktree file equals the current
  source file — together the worktree's effective hash equals the studied one.
- **Both current-hash fields earn their place.** `current_content_sha256` drives
  editability and the missing-source message; `current_source_sha256` drives the
  worktree byte guard. Neither is dead, and collapsing them would force Swift to
  re-wrap prompts — the exact duplication this pass removed.
- **The identity revalidation is sound.** `refinementWorkspaceIsCurrent` rejects
  any post-selection drift in Wave, repo, Task id/identifier, Task Session,
  Project Session, workspace slug, branch, or normalized worktree path; the
  terminal is created only after every guard passes and is closed on a failed
  command dispatch. The route now targets `.terminal` via a typed
  `initialSection` that replaced the `opensAgent` boolean.
- **`[focus] + descendants(of: focus)` is duplicate-free** because `descendants`
  starts from children; a leaf focus now populates the table instead of showing
  an empty parity view.
- **DTO discipline holds.** `current_source_sha256` is explicit `Option`/`String?`
  with no serde default, is present in the shared fixture, and is asserted by the
  Swift fixture test; the Rust round-trip fixture test still passes.

One cosmetic inconsistency was left in place deliberately: the refinement seed
still instructs the agent to refuse edits when the *effective* hash drifts, while
the launch guard enforces raw-byte equality. It is soft guidance the agent cannot
cheaply recompute anyway, and tightening the wording would add churn without
changing behavior.

Checks run this pass, all green: `cargo fmt --check`; `cargo clippy -p loopflow
--all-targets -- -D warnings`; `cargo test -p loopflow --lib context` (66 tests,
including `smooth_representative_excludes_steered_launches`,
`current_source_hashes_keep_file_and_effective_identity_separate`, and the Swift
contract round-trip); `scripts/check_migrations.py` (chain intact through
`0.11.006_context_launch_work`); `swift test` (117 tests, including the
selected-source-share, workspace-recheck, and source-byte-hash tests);
`scripts/check_swift_multiplatform_boundaries.py`; and `xcodebuild
build-for-testing` for LoopflowMac (**TEST BUILD SUCCEEDED**). This pass changed
no product code, so Python, website, E2E, and hosted UI suites were not rerun.

## Second independent pass (2026-07-15): representative dedup

A fresh reviewer re-read the committed feature diff (`main...HEAD`, tip
`ec28945b`) end to end against the design's "Done when," re-deriving rather than
restating the prior claims. The read reconfirmed exact flame summing, the
smooth-representative filter, ratio-based selected-source-share sorting, the
effective/source hash split, the ordered pre-terminal handoff guard, per-launch
candidate dedup, and DTO discipline. It surfaced one genuine correction the
earlier passes missed.

**Correction — representatives no longer repeat one address across roles.**
`select_representatives` chose each of the four roles (smooth complete,
high-context complete, failed/steered, recent) independently. On a small
revision where a single completed, complete-capture, zero-steer launch is
simultaneously the smoothest, the highest-context, and the most recent, the same
`TraceAddress` was emitted three times. The evidence rail renders one row per
representative (role + run-id short hash + Open trace), so that population would
show one session as three "independent" pieces of evidence — a false read of the
evidence base the design describes as distinct sessions. The function now claims
addresses in priority order and skips any role whose winning candidate was
already claimed, so a lone qualifying run surfaces exactly once under its
highest-priority label. Populations large enough to have distinct smooth,
high-context, failed/steered, and recent traces are unchanged: on the live
`LOOPFLOW.md` population the smooth and high-context representatives were already
different traces, so this only collapses the tiny-population duplication.

New test `representatives_never_repeat_one_address_across_roles` builds a
single completed/zero-steer launch and asserts exactly one representative
(`SmoothComplete`). `smooth_representative_excludes_steered_launches` and the
rest of the context suite still pass.

The seed-wording versus byte-guard inconsistency from the prior pass was left as
deliberate soft guidance; nothing else in token attribution, flame summing,
revision identity, the handoff guard, or DTO discipline warranted a change.

This pass did **not** advance the still-missing continuous journey: no real
Intelligence Task refinement, source diff, lifecycle, or backlink was exercised,
and none is claimed here. See Risk 1 and the Refinement-truth audit below.

Checks run this pass, all green: `cargo fmt --check`; `cargo clippy -p loopflow
--lib -- -D warnings`; `cargo test -p loopflow --lib context` (67 tests,
including the new `representatives_never_repeat_one_address_across_roles` and the
Swift contract round-trip); `scripts/check_migrations.py` (chain intact through
`0.11.006_context_launch_work`); and
`scripts/check_swift_multiplatform_boundaries.py`. The change touches only Rust
aggregation semantics and adds no wire field, so the committed pass's Swift suite
(117 tests) and `xcodebuild build-for-testing` still hold and were not re-run;
Python, website, E2E, and hosted UI suites were likewise untouched.

## Risks and bottlenecks

1. **The refinement loop is not yet live.** The PM cache contains W2-71 under
   Intelligence / Context, but `intelligence` is not registered and no W2-71
   Task Session or worktree exists. Context Lab cannot manufacture that control
   ownership in Swift.
2. **Historical operating-guide rows remain intentionally read-only.** They
   predate source-path capture and show “No canonical file source.” Fresh
   canonical capture is now proven separately and must not rewrite those rows.
3. **Historical Project and Task attribution stays missing.** Only launches
   captured after migration `0.11.006_context_launch_work` can populate those
   facets; no backfill guesses ownership from filenames or worktree names.
4. **Task creation is omitted.** The current sheet selects only an inactive Task
   Session with a durable worktree; it cannot create a Linear Task and return a
   workspace receipt in one human-confirmed operation.
5. **Native startup logs AttributeGraph cycles.** No visible Context Lab failure
   has been attributed to them yet; the final demo must settle that rather than
   normalize noisy runtime diagnostics.
6. **The old isolated development ledger is divergent after rebase.** It is not
   used when the dev app explicitly opts into `/Users/jack/.lf/loopflow.db`, but
   a default dev launch will still fail its local reader until that disposable
   store is rebuilt under the 006 chain.
7. **The installed release CLI is now behind the live ledger.** The explicit
   production-data pass upgraded the ledger to 006; installed `lf` knows 005 and
   correctly refuses the newer schema. The branch binary reads it cleanly, and
   the automatic 005 backup is
   `/Users/jack/.lf/loopflow.db.backup-0.11.005_provider_accounts`.
8. **The filter rail does not yet cover the whole design.** Steering is visible
   as a denominator and lane sort but cannot define a steered-only population;
   revisions are always all revisions rather than current-versus-all. The Rust
   query remains atomic for every filter it does expose.
9. **Revision comparability is intentionally narrow.** The gate requires at
   least three exposed launches per revision and complete-capture rates within
   ten percentage points. It does not yet compare provider/model mix or balance
   observation windows, so passing the gate is evidence coverage, not proof of
   causal comparability.

## Done-when audit

- **Research truth — code and live reader hold; hosted proof remains open.** A
  fresh 30-day production-ledger query reconciles supplied tokens, root width,
  child widths, and missing lane totals exactly. Atomic snapshot replacement,
  query cancellation, shared lane scale, selected-share sorting, and
  flame/table identity are implemented and tested. This pass did not run the
  installed app or hosted keyboard journey, and the missing steering/revision
  filters keep “any useful session set” from being fully proven.
- **Evidence truth — holds at the reader/model boundary.** Canonical revisions,
  representative roles, exact full hashes, artifact availability, explicit
  exact-trace opening, and a fresh editable canonical `LOOPFLOW.md` capture are
  present. Missing-artifact rows can still open the address and report the
  actual absence rather than disabling the path.
- **Refinement truth — guarded code path, not continuous proof.** The structured
  seed and stale source/workspace guards exist; Task, Project Session, branch,
  and worktree identity are revalidated before terminal creation, and the route
  targets the terminal receiving the fresh process. A real Intelligence Task
  launch, source diff, lifecycle, and backlink have not been experienced.
- **Learning truth — natural grouping holds; intervention proof remains open.**
  Revision comparison runs over naturally observed hashes and shows a concrete
  coverage blocker on real data. Its current comparability rule is limited, and
  the edit → ordinary run → new canonical hash journey remains undemonstrated.
- **Shipping proof — proportional final checks pass.** This pass ran 117 Swift
  tests, 8 focused aggregation tests, 21 context integration tests, the canonical
  migration registration test, Swift multiplatform boundaries,
  `scripts/check_migrations.py`, `cargo fmt --check`, and
  `cargo clippy --all-targets -- -D warnings`. The signed macOS app and UI-test
  runners also completed `xcodebuild build-for-testing`. Python, website, E2E,
  and hosted UI behavior were not rerun because this uncommitted pass changed
  only Rust aggregation semantics and Swift app behavior; their prior branch
  proof is not promoted into a new continuous-demo claim.

## What is intentionally not included

- a public instruction-admin CLI;
- a copied prompt database or embedded Markdown editor;
- an LLM-authored quality score;
- remote telemetry or automatic prompt-body opening;
- guessed Project/Task identity.
