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
evidence base the design describes as distinct sessions. That pass claimed exact
addresses in priority order and skipped a role when its winning address had
already been claimed. The next independent pass below strengthens the identity
boundary from address to session and fills roles from the next-best distinct
session when possible.

The original single-launch regression is now the strengthened
`representatives_never_repeat_one_session_across_roles` test described below.

The seed-wording versus byte-guard inconsistency from the prior pass was left as
deliberate soft guidance; nothing else in token attribution, flame summing,
revision identity, the handoff guard, or DTO discipline warranted a change.

This pass did **not** advance the still-missing continuous journey: no real
Intelligence Task refinement, source diff, lifecycle, or backlink was exercised,
and none is claimed here. See Risk 1 and the Refinement-truth audit below.

Checks run this pass, all green: `cargo fmt --check`; `cargo clippy -p loopflow
--lib -- -D warnings`; `cargo test -p loopflow --lib context` (67 tests,
including the original exact-address regression and the Swift contract
round-trip); `scripts/check_migrations.py` (chain intact through
`0.11.006_context_launch_work`); and
`scripts/check_swift_multiplatform_boundaries.py`. The change touches only Rust
aggregation semantics and adds no wire field, so the committed pass's Swift suite
(117 tests) and `xcodebuild build-for-testing` still hold and were not re-run;
Python, website, E2E, and hosted UI suites were likewise untouched.

## Third independent pass (2026-07-15): session evidence and launch receipts

This pass re-read the full feature diff against every “Done when” checkpoint,
then reviewed the uncommitted representative change as production code rather
than accepting its prior rationale. It found two correctness gaps and one small
persistence hazard.

**Correction — representative uniqueness now means one real session.** The
evidence copy promises representative sessions, but the prior dedup key was the
full `TraceAddress`. One outer Loopflow session can contain several launches and
turns, so it could still occupy several roles under different addresses. The
prior algorithm also discarded a role when its first choice was already claimed
instead of looking for the next-best independent session. `select_representatives`
now claims `run_id` in role priority order and filters claimed sessions before
each selection. A one-session population produces one row; a larger population
retains smooth, high-context, failed/steered, and recent roles whenever distinct
eligible sessions exist. The strengthened
`representatives_never_repeat_one_session_across_roles` test covers multiple
addresses from one run, while
`representatives_fill_roles_from_distinct_sessions_when_possible` proves the
fallback rather than merely proving deletion.

**Correction — the source receipt is rechecked at the last safe moment.** The
handoff already refreshed Context Lab evidence and revalidated the chosen Task
workspace, but the canonical source could still change while roadmap and Task
state were being resolved. The launch path now rehashes both the canonical file
and the mapped Task-worktree file against Rust's fresh raw-source receipt after
terminal creation and immediately before dispatching `lf refine`. Any mismatch
closes the new terminal and returns a concrete refresh/rebase repair path. No
agent command runs against a stale starting receipt. The Swift hash test now
also proves that a post-receipt file mutation is detected.

**Simplification — persisted visualization values are semantic.** Saved views
and deep links now store `aggregate`, `lanes`, or `table`; user-facing labels
come from a separate `title`. Renaming a tab no longer invalidates persisted
research state. An unused `CryptoKit` import was removed at the same time.

A fresh branch-binary query of the production 30-day ledger found 54 sessions,
127 launches, 135 turns, and 127 assembled turns. Eight provider-total-only
turns remain coverage-only. Attributed context, aggregate-root width, and the
sum of root children all reconciled at 1,003,087 tokens; no lane with attributed
assets lacked a supplied total. The current population reports $9.67248525 over
nine cost-captured turns and outcomes of 100 completed, four failed, one
interrupted, and 22 running launches. Evidence contains no repeated
representative `run_id`.

The natural editable `LOOPFLOW.md` revision remains effective hash
`130b91c3afb3afa7897e22cb85068a1714ab6431469dee3392eda10eb8bdd4fe`.
It now has eight exposed launches and 9,320 attributed tokens in the moving live
population, with distinct smooth, high-context, and recent session
representatives. These changing counts are a current reader snapshot, not a
before/after intervention result.

This pass did **not** launch a real Intelligence Task refinement, edit a source,
observe its Task diff, follow the backlink, or run a natural post-edit session.
The installed-app keyboard journey and hosted UI runner were also not available
in this headless environment. None of those missing proofs is inferred from
unit, model, or build success.

Final checks all passed: `uv run python scripts/test.py --all` ran 57 Python
tests, 1,329 Rust tests (three skipped), 59 website tests (three skipped), 117
Swift tests, the E2E smoke test, Swift multiplatform boundary validation, and
LoopflowMac `xcodebuild build-for-testing` (**TEST BUILD SUCCEEDED**). After the
saved-mode regression was added, the full Swift suite was rerun at 118 tests
across 22 suites. Focused checks also passed: 68 context-filtered Rust tests,
seven Context Lab Swift tests, `scripts/check_migrations.py` through
`0.11.006_context_launch_work`, and `git diff --check`. The E2E run repeated the
known warning about the divergent disposable development ledger; it did not
touch the explicitly selected production ledger used for the reconciliation.

## Fourth implementation pass (2026-07-15): research-state cohorts

This pass closes the two remaining computable filter gaps and the known
revision-comparison confounds without touching the externally blocked Task
refinement journey.

**Steered-only is an observed launch predicate.** `SessionSetQuery` now carries
the required `steered_only` boolean through Rust, the shared fixture, Swift,
deep links, and saved views. Rust loads candidate launches and turns, then
retains only launch ids with a captured `input_op == "steer"` before building
totals, coverage, lanes, flames, or evidence. A launch with no observed steer
does not silently qualify. The filter rail names this **Observed steering
only**, and `lf context --steered-only` exposes the same reader contract.

**Current-revision-only is a canonical-source predicate, not a Swift hash
guess.** The required `current_revision_only` boolean reaches Rust as
`--current-revision-only`. Rust canonicalizes each resolvable file-backed
instruction source, computes its kind-specific effective hash, and retains
launches containing at least one captured revision that matches the file now on
disk. Missing paths, unreadable files, goal/memory composite state, and
source-less assets do not count as current. Once a launch qualifies, its whole
context remains in the atomic snapshot so flame widths and session totals still
reconcile; the UI therefore says **Contains current file instruction** rather
than implying every asset in the launch is current.

**Comparison cohorts now carry their confounds.** Every revision measurement
adds required `last_seen` and `provider_models` fields. Provider/model buckets
count distinct exposed launches; `model: null` is an explicit bucket, while a
missing or incomplete bucket sum blocks comparison. Swift keeps the existing
three-launch minimum and ten-percentage-point complete-capture limit, then also
requires provider/model total-variation distance at or below 20 percentage
points and non-zero observation spans within 2× of one another. Missing windows,
single-time populations, imbalanced spans, and imbalanced mixes each render a
concrete unavailability reason. No wire field has a serde or Swift default.

A final branch-binary read of the production 30-day ledger reconciled 55
sessions, 131 launches, 139 turns, and 1,053,450 attributed tokens; the aggregate
root and sum of its children both equal 1,053,450. The observed-steering cohort
contained two sessions, three launches, 11 turns, eight steer turns, and 31,318
attributed tokens. The current-file-instruction cohort contained three sessions,
three launches, three turns, and 30,982 attributed tokens. Across all 245
revision evidence rows, provider/model bucket counts summed to each revision's
exposed-launch count and every row carried first and last observation timestamps.

The moving ledger also exercised the negative current-file case. The captured
`LOOPFLOW.md` revision now has 12 exposed launches and 13,980 attributed tokens,
but the canonical file's effective hash has changed. It remains visible as
historical evidence and does not count as a matching current revision. The
current-file cohort instead qualifies through the naturally captured current
`.lf/skills/compress.md` revision. None of this is presented as an intervention
or causal-comparison result.

The final proportional gate ran
`uv run python scripts/test.py --base bdc22c8aa --loopflow`: Rust formatting,
clippy with warnings denied, and 1,332 Rust tests passed; the website ran 59
tests with three skips; Swift ran 119 tests across 22 suites; the multiplatform
boundary check passed; and LoopflowMac plus its UI-test runner completed
`xcodebuild build-for-testing` (**TEST BUILD SUCCEEDED**). Migration-chain
validation also passed through `0.11.006_context_launch_work`, and
`git diff --check` was clean. Python and E2E were not selected because this
slice changes neither surface; the full-matrix gate recorded in the preceding
pass remains the baseline for those paths. The hosted UI gate remains unavailable
in this headless environment.

The real Intelligence Task launch, source edit, Task diff, lifecycle, backlink,
and natural post-edit revision remain unclaimed and blocked exactly as before.

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
## Done-when audit

- **Research truth — reader truth holds; hosted interaction proof remains
  open.** The fresh 30-day production query reconciles supplied tokens, root
  width, child widths, and missing lane totals exactly. Atomic snapshot
  replacement, cancellation, shared lane scale, ratio-based sorting,
  flame/table identity, observed-steering filtering, and canonical
  current-revision filtering are implemented and tested. The installed-app
  keyboard journey was not run.
- **Evidence truth — holds at the reader/model boundary.** Canonical revisions,
  distinct representative sessions, full hashes, artifact availability, and
  explicit exact-trace opening are present. The prior live body read remains
  valid evidence; this pass did not reopen prompt or conversation bodies merely
  to repeat it. Missing-artifact rows still open their exact address and surface
  the absence explicitly.
- **Refinement truth — launch guards are stronger; continuous proof is still
  absent.** Task, Project Session, branch, worktree, and source identity are
  revalidated, and canonical plus Task bytes are checked again immediately
  before command dispatch. Failures remove the new terminal. No real
  Intelligence Task launch, source diff, lifecycle action, or backlink was
  experienced.
- **Learning truth — natural grouping and conservative cohort gates hold;
  intervention proof remains open.** Revision comparison operates on naturally
  observed hashes and explains insufficient launch count, capture parity,
  provider/model mix, missing or single-time observation data, and imbalanced
  spans. Passing those gates permits measured comparison; it does not prove
  causality. The edit → ordinary run → new canonical hash journey remains
  undemonstrated.
- **Shipping proof — the full local matrix passes.** Python, Rust, website,
  Swift, E2E smoke, multiplatform boundaries, migrations, and Mac
  build-for-testing all passed. The hosted UI runner and installed-app demo were
  not run; build success is not promoted into interaction proof.

## What is intentionally not included

- a public instruction-admin CLI;
- a copied prompt database or embedded Markdown editor;
- an LLM-authored quality score;
- remote telemetry or automatic prompt-body opening;
- guessed Project/Task identity.
