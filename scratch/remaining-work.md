# Remaining work to code-complete review

LOO-298 · Reordered 2026-09-29 on Jack Heart's direction: "do the deepest cuts
first." Work the list top down. Items below it keep their detail in the
sections that follow.

## Order, deepest first

1. **Every step uses its ordinary lf command.** Skill, Task skill, router and
   review run `lf skill`; ops use child command dispatch. The Task and saved-Flow
   launchers are deleted locally. Close the remaining control/review/account
   proofs and Task preflight policy exception in the
   [before/after inventory](exec-per-step.md#direct-skill-command-cut--2026-09-29).
   Definitions compile fully before execution; authored subflows are display
   provenance only. Captured input remains Session history, not another process.
2. **Simplify attribution and the parent tree.** Jack: "make sure that we
   simplify and clarify attribution and the parent tree after this." Once a step
   is an Exec, list every parent pointer and every Task/Wave attribution field,
   name the single owner of each fact, derive the rest, delete the copies.
3. **Names follow the model.** Jack: use the word Exec instead of launch or run
   wherever the thing is one agent start under one lf process. Recorder types
   that say Run are renamed on that basis (finding 2). Own commit.
4. **Saved-Flow discovery with Desktop paging and reconciliation.** In flight
   when the order changed; checkpoint it when coherent, then return to item 1.
5. **Released-populated import**, executed through the public binary and on a
   materialized copy.
6. **Docs, skills and generated pages** for final behavior.

Runtime loop children, structured-result Flow decisions and captured events have
hosted verification. History readers pass hosted Rust at `142314682`; its two
Swift embedded-fixture failures are repaired with the discovery slice.

## Detail retained from the earlier checklist

LOO-298 · 2026-09-28 · Consolidated for Jack Heart. This is the scope checklist,
not evidence that each preceding implementation has passed.

## Docs and README now supply the spec

Jack requested this order on 2026-09-28: rewrite the docs and README for the
accepted model before further executable owner conversion. The initial spec
rewrite, originally `4bb44b999`, is retained in the published branch. It covers:
`README.md`, `docs/index.md`, `docs/lf.md`, `docs/agent-api.md`, `docs/waves.md`,
architecture/reference/data/execution, and other active pages that define these
objects. Derived docs were regenerated through the existing generator.

Exec is the actual lf process; AgentSession is the continuable conversation;
FlowSession is captured resumable Flow progress. Their relationships and history
replace the separate Run product object. Keep one explicit implementation-status
section so the target contract does not misrepresent unfinished behavior. Keep
current command spellings and source inventories accurate during conversion.
Architecture Reference now maps the spec to the remaining implementation;
writing the spec does not complete the requirements below. The metric-planning
repair is published; execution-owner conversion continues below.

The eight Chapter builtin skill updates, including both chapter reviews, are
integrated. They use repository rotation and Linear Project ownership.
The restart hint in `ops/task.rs` names missing synced Project data
and Wave planning sync, removing the obsolete chapter command. Verify it with
the Rust consistency pass. Preserve historical migration text. The checked
ownership inventory must not list deleted Chapter storage as a current dependency.

## 1. Finish the execution owners

Main is converting Flow settlement to exact AgentSession history; see
[handoff](parallel-execution.md). Keep one shared Task/taskless driver and Task's
one managed FlowSession pointer while allowing other attributed Flows.

The 2026-09-29 runtime-child slice implements backward-edge entry, parent wait,
exact child return, same-child retry and next-pass sibling allocation through the
shared driver/transaction. Source, materialized and scripted public evidence is
recorded in [the handoff](parallel-execution.md#runtime-children-and-ci-repairs--implementation-iteration-11).
Keep those proofs through final discovery/history/import integration. They do not
settle configured recovery or the separate native retry-contract failure.

- Select the exact native start under Flow version/claim; consume its matching
  success once with cursor advancement. Failure, an older success, a helper,
  stale driver, late generation or unrelated continuation cannot settle it.
- Prove failed then successful continuation in the same conversation, source-free
  recovery, XOR/runtime nesting, and driver death before native completion.
  Retain the command's unknown outcome when provider completion is known.
- Preserve automatic provider retries within one lf command. Source review found
  `launch_agent` captures Flow selection once, the retry loop clones it for each
  harness, and `select_flow_turn` refuses a different selected start. A successor
  native turn after an automatic provider retry needs authorized reselection with
  prior failure/usage retained. The automatic-retry native probe first reproduced
  success leaving the Flow stuck on its failed turn, then passed on candidate
  84fb1f7d. Preserve that proof and the live/successful-selection exclusion when
  completing owner removal. See evidence; no new attempt object.
- Decision/router candidates must belong to the successful selected turn.
  Supervisor's candidate84fb1f7d CLI probe reproduced a failed Advance surviving
  retry. Candidate894aded0 repairs it: missing-decision retry stops, a retry may
  replace failed Iterate with successful Advance, and history survives. Retain
  these native proofs and the route-clearing invariant through Run removal;
  XOR router replacement has not been separately replayed.
  The delayed-child case is now a reproduced failure, not just a proof gap:
  `supervisor-late-decision-retry-3` lets a failed turn's child decide after the
  retry starts, and the Flow wrongly completes. Preserve original native-turn
  authority through child commands; version/current Run alone cannot distinguish
  automatic retries. See evidence and Task comment `cae3e281-31e1-4ba0-a757-3dd2cb00c4ee`.
  Jack's 2026-09-28 clarification supports bounded investment in this reproduced
  failure while rejecting complexity driven by imagined races. Preserve original
  turn attribution using the existing owners and one shared navigation check;
  delete superseded Run authority. No additional attempt object or general race
  framework. Substantial provider-specific machinery warrants revisiting the
  retry boundary rather than silently widening the repair.
- Mechanical boundaries retain correlated start/outcome directly on FlowSession.
  One lf process may perform several operations; no fake Exec or AgentSession.
  Missing completion keeps inspect-before-retry semantics.
- Complete Run removal: attribution, launch publication, selected member/history,
  provider/account identity, artifacts and outcomes all need their final owners.
  Delete `runs`, `current_run_id`, old reader/writer paths and replacement wrappers
  only after preservation and callers converge. Renaming tables is intermediate.
  Published admission now owns captured input/publication on AgentSession and
  removes the Session reader's Run join. Flow publication and selected native
  history use that owner. General history/usage/activity and landing-repair
  readers now select retained conversation inputs, preserving each input's
  attribution and chronology. These are intermediate wire projections, not
  complete Run deletion. Retained SQL rows and remaining callers still require
  preservation and conversion; no runtime Run writer should be restored.
  Verdict/route/blocked now share original-turn caller Exec authority. Focused
  source/canonical tests cover those store changes, while the real provider's
  valid decision retry remains failing. The public automatic-retry ownership
  fixture passes on candidate `a1397de2c6f3`, including public usage of 40 input
  and 10 output tokens, unknown cost, both outcomes and one consumed success.
  Six Session CLI and five cutover checks separately cover Ask/review paths;
  populated source/canonical admission checks retain their recorded limits.
  None proves the complete managed/provider matrix or valid native decision
  retry. Complete the remaining launch/publication and historical consumers.
  Published `lf/commands/flow.rs::run_op` records mechanical results directly
  on Flow history. Focused source/canonical, standalone crash/retry and hosted
  disposable-OS managed proofs pass within their recorded scope (see evidence).
  Convert this complete boundary with TaskLauncher and saved-launch callers;
  preserve exact selected native history and uncertain mechanical effects.
- Finish Ask/keyed answer, review completion, helpers (rebase conflicts/landing
  repair), replay, usage, activity and Task worker paths. Recording and resumability
  apply to headless agents as well as interactive ones.
- Preserve the observed Task-settlement counterexample in evidence: a Python
  inspection error was classified as a capability blocker because it printed an
  old fixture log containing `operation not permitted`. Quoted command output
  cannot become current control authority. Keep actual delivery/provider failures
  on their outcome owners. The source repair and focused proof are published;
  captured installed control still uses its earlier binary. Preserve the repair
  through final integration without command allowlists or another classifier.

## 2. Native continuity and authority

Retained transport proofs use real Codex 0.157.1 with synthetic Responses, private
Homes and controlled clients. They do not establish configured account continuity.
The supported native Unix WebSocket route avoids a general service; a narrow
conversation bridge must not recreate deleted Wave listeners/residents/lfd.

- Recorded native Home/account must survive reconnect/retry; do not select today's
  account and claim old-thread continuation. Every supported provider path needs
  usable lifecycle evidence. At the audit, only Codex recorded Session native
  process/thread while TaskLauncher claimed all providers; Claude retained an old
  Run sidecar. Do not narrow the product contract just to fit a Codex fixture.
- Explicit retry after both driver and engine crash now passes the standalone
  public CLI probe on candidate6823ef08, retaining unknown earlier Exec outcome,
  native thread and history. Current source prepares both Flow and Task retries
  through shared recovery before claim acquisition. Prove the managed path and
  combined final bytes with Task claim/agent/unblock policy; see evidence.
- OpenCode headless/managed native history now uses the common harness and
  existing Session/Flow fences. Public scripted-server checks cover native
  decision success, earlier-caller rejection on automatic retry, exact consumption,
  retained usage/final output, and uncertain completion after a tool effect.
  Actual OpenCode/scripted-model output proof passes within its recorded scope.
  Interactive reconnect, configured accounts, and complete managed/provider
  recovery remain required; this does not settle Codex valid native retry.
- Busy surviving turns must be observed/recovered without accidental extra input.
  Session driver handoff and Flow orchestration claims remain independent.
- Explicit restart uses exact process identity and exclusive ownership, retains
  conversation/history, fences stale clients and preserves shared-engine siblings.
- Recover completion after all clients/driver die, exactly once. Preserve original
  generation/Exec attribution and missing usage; gen1 missed usage recovered by
  gen2 still needs its precise proof. Existing recovered-outcome proof does not
  establish recovered usage or a real database write failure.
- Reservation/artifact publication crashes reconcile immutable saved input and
  launch once. Required conversation rows must exist before provider side effects.

## 3. Exec admission and discovery

Every actual lf process should be observable with trustworthy command outcome.
Parsed ordinary commands record Execs. Early help/version/parser exits now use
`with_process` observation through an existing compatible ledger; install and
screenshot observe before dispatch. Observation neither initializes nor migrates
a store, and absent/incompatible history stays explicit without blocking the
command. This implemented boundary does not need rebuilding.

Source assertions and the retained hosted 94b Rust job109542699794 cover
`early_commands_record_exact_exits_without_initializing_or_migrating` (1.038s),
`early_observation_preserves_preflight_target_and_screenshot_child_ancestry`
(1.176s), and `parser_returns_exact_status_without_admitting_an_early_store`
(0.075s). They retain code 0/0/2, zero AgentSessions, unchanged schema,
incompatible target bytes and distinct screenshot child/parent Execs. The
2026-09-29 concept review inspected those assertions; it ran no new tests.
This is not every-entry-gate, live installation or all-signal proof; those limits
remain. Best-effort command observation is not required agent-launch admission.
Never invent observed start, exit or signal for an incomplete journal.

Use typed summary/detail queries for Exec, AgentSession and FlowSession. Filter
repo, Task/work evidence, identity, parent, command, mode, title/skill and state
before payload I/O. Distinguish command context from work performed; observational
commands must not start Tasks. One Exec can perform work for several Sessions.

Session inventory no longer joins Run. Remaining audit gaps: final bounded
enrichment/pagination and substring-search behavior, Flow inventory decoding
complete captures, and the remaining indexed discovery consumers. Exec summary/detail now has a
bounded public `lf exec list/show` consumer, typed Rust/Swift fixture and focused
store/public proofs. Preserve
missing-payload rows. Current conversation history filters Work, caller and date
in SQL before decoding selected evidence, but that is not the dense-data proof.
Activity ending within a window and recent-start history are different queries.
Resolve historical identities independently of launch eligibility/current PRs.

Bounded cursors/prefix queries are engineering proposals, not accepted changes to
contains-search semantics. Document actual semantics and plans. No transcript
FTS is needed. Prove fixed-dataset pagination, ambiguous prefixes, rename stability,
mode/repo scope and bounded enrichment; Desktop currently uses unlimited inventory
to avoid offset races. Replace that caller together with its paging contract.

Measure representative dense data (audit suggested 100k Execs/20k Sessions/5k Flows,
three repos/1k Tasks; not a mandatory product limit). Report fixture scale, source,
host load, cold/warm list/detail median/p95 and SQL plans; separate startup/schema,
SQL and payload cost. The recorded Exec dense probe now covers the suggested
scale with whole-process timings (p50 314–339ms, p95 320–341ms); uncontrolled OS
caches and unseparated startup/query cost do not establish cold-cache or final
Session/Flow/Desktop acceptance. An empty-list improvement is not this proof.

## 4. Import, binding, Started

Follow [import preservation](import-preservation.md), including conflicts and
interrupted conversion. Strict nullable ancestry, same-target/done-Task bind,
racing bind/replacement and monotonic first-assignment timestamp remain required.
Prospective usage attribution is an explicit [assumption](questions.md), not a
new Jack decision. No bulk history rewrite; mid-turn missingness stays visible.

## 5. Desktop, wire and Chapters

- Move Rust/Swift DTOs and fixtures together: typed ancestry, AgentSession history,
  Exec command outcome and final numeric graph IDs. Numeric graph/membership IDs
  now pass shared Rust/Swift fixtures, nested containment/return counts, public
  CLI/import, managed Task topology and mounted terminal retention checks. Final
  Session/Exec history and discovery wire conversion remains required.
- Bound Tasks omitted from the roadmap must retain Task ancestry/breadcrumbs,
  Session identity and pane. Interactive default and explicit headless/completed
  discovery must agree with CLI; `--all` continues to mean all repositories.
  The bounded Swift contribution now implements known ancestry/selection and
  orphan classification using existing typed wire fields. Main passed its 25-test
  navigation suite and extended mounted terminal proof on the handback bytes;
  planning disappearance/return preserves the selected surfaces and draft there.
  Retain this proof through the final DTO conversion. Headless discovery was
  subsequently integrated with complete-inventory filtering
  and permanent-bind preview/confirmation. The focused mounted proof retains
  terminal/draft/focus across toggles and binding to a Task in another checkout.
  Configured Desktop acceptance and final paging remain separate requirements.
  See `.lf/tmp/cut-i/desktop-ancestry-handback.md` for exact checks and hashes.
- Prove selected terminal surface/draft survives rename/bind/refresh; conversation
  identity survives restart. Native thread persistence alone does not prove an
  unsubmitted editor draft. Projection updates only when inputs change.
- Complete [Chapter/default Flow](chapters.md) integration and preservation.

## 6. Existing incidents and delivery

Publication identity repair is integrated with focused failure proofs: persist
acknowledged GitHub identity before optional follow-up/Linear enrichment, after
reconciling existing pinned merge intent. PR1296 association is now observed;
original missing-association cause remains unknown.

Existing-root Task stacking is integrated: parent selection preserves original
fork, IDs, authored checkout, PR/publication and Flow history; ordinary rebase
integrates from that fork. Same-parent retry uses the retained parent PR. Different
existing parent remains separate reparenting scope. Retain held-claim exclusion,
locked base reread and narrow writes preserving new parent plus refreshed PR facts.
Do not replay archived handoff patches.

Cancellation/refused start: preserve preflight before issue creation and retryable
identity after post-create allocation failure. LOO-305 owns provider deletion;
that operation does not certify process termination. Retain the released-before-
stop counterexample: claim released before stop, live exact-owned child missed.
Reproduce against the new process owners if claiming settlement; never use causal
ancestry alone to signal. No duplicate deletion implementation or widened scope.

## 7. Integrated finish

Preserve main's upstream semantics and rebase/publish coherent verified checkpoints
through lf. Run affected checks after final changes, canonical materialization in
a disposable source copy, architecture/migration checks, fmt and all-target Clippy.
Current publication/base and exact CI receipts belong in
[the control index](parallel-work.md); main owns subsequent integrations.
Preserve upstream Session executable/Home/database selection, launch-failure
journal evidence, landing/release observation, schema cache and resource cleanup
semantics. Focused reconciliation receipts remain in [evidence](evidence.md) and
[the main handoff](parallel-execution.md).

The materialized diagnostic ran all selected cases: 1,925 passed, 33 failed,
15 skipped. Later focused repairs do not turn that mixed snapshot into a full
green matrix. Preserve its exact source receipt and failure dispositions; rerun
affected failures, not a full matrix after each small correction. Hosted CI is
still owed. Implement iteration 10 remains active; no review edge or requirement
is discharged by publishing a checkpoint. The pre-curation checklist is retained
at `da19cf0b5:scratch/remaining-work.md` for its historical receipt chronology.
Do not cite assertion passes with a leaked process as clean settlement. Review
complete public behavior and deletion paths; measure code by the same method/base.
Update docs, skills and generated HTML with actual behavior. Compress and follow
the saved Flow's review boundaries; code-complete concept review is the requested
stop. Configured Desktop/provider, real-Home upgrade and release acceptance remain
separate explicit obligations with a prepared procedure, not fabricated passes.
