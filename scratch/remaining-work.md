# Remaining work to code-complete review

LOO-298 · 2026-09-28 · Consolidated for Jack Heart. This is the scope checklist,
not evidence that each preceding implementation has passed.

## Docs and README now supply the spec

Jack requested this order on 2026-09-28: rewrite the docs and README for the
accepted model before further executable owner conversion. The recovery
checkpoint and mechanical conversion are rebased and published at `fb100f486`
against main `1dce02734`. The initial spec rewrite, originally `4bb44b999`,
survives that rebase. It covers the active guides:
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

The bounded Chapter contributor updated eight active builtin skills, including
both chapter reviews, to repository rotation and Linear Project ownership.
Supervisor reviewed its uncommitted diff and static checks; main owns integration.
The working restart hint in `ops/task.rs` now names missing synced Project data
and Wave planning sync, removing the obsolete chapter command. Verify it with
the Rust consistency pass. Preserve historical migration text. The checked
ownership inventory must not list deleted Chapter storage as a current dependency.

## 1. Finish the execution owners

Main is converting Flow settlement to exact AgentSession history; see
[handoff](parallel-execution.md). Keep one shared Task/taskless driver and Task's
one managed FlowSession pointer while allowing other attributed Flows.

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
  Current dependency is concrete: `store/sqlite/flows.rs::FLOW_SELECT` still
  reads Run publication/outcome, `publish_attempt` writes it, and creation of
  the native-selection capability reads Run. The working conversion has removed
  the Run join from `select_flow_turn`; it carries the reserved AgentSession ID.
  Verdict/route/blocked now share original-turn caller Exec authority. Focused
  source/canonical tests cover those store changes, while the real provider's
  valid decision retry remains failing. The public automatic-retry ownership
  fixture now asserts zero Runs and fails on one retained row after consuming
  the successful native turn; launch/publication conversion owns that boundary.
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
  on their outcome owners; remove this ambiguity during the Task/Exec conversion
  without introducing command allowlists or a second failure-classification lifecycle.

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
Parsed ordinary commands currently record Execs; help/version/rejected arguments,
installation/preflight and screenshot paths need explicit final disposition.
Best-effort command observation is not required agent-launch admission. Never
invent observed start, exit or signal for an incomplete journal.

Use typed summary/detail queries for Exec, AgentSession and FlowSession. Filter
repo, Task/work evidence, identity, parent, command, mode, title/skill and state
before payload I/O. Distinguish command context from work performed; observational
commands must not start Tasks. One Exec can perform work for several Sessions.

Remaining audit gaps: Run lists hydrate before capping; Session inventory still
joins Run and uses substring search; Flow inventory decodes complete captures;
Exec has no complete indexed summary/detail API. Preserve missing-payload rows.
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
SQL and payload cost. An empty-list improvement is not this proof.

## 4. Import, binding, Started

Follow [import preservation](import-preservation.md), including conflicts and
interrupted conversion. Strict nullable ancestry, same-target/done-Task bind,
racing bind/replacement and monotonic first-assignment timestamp remain required.
Prospective usage attribution is an explicit [assumption](questions.md), not a
new Jack decision. No bulk history rewrite; mid-turn missingness stays visible.

## 5. Desktop, wire and Chapters

- Move Rust/Swift DTOs and fixtures together: typed ancestry, AgentSession history,
  Exec command outcome and final numeric graph IDs. The nested graph projection
  repair retains structural wire keys; it is not the numeric DTO conversion.
- Bound Tasks omitted from the roadmap must retain Task ancestry/breadcrumbs,
  Session identity and pane. Interactive default and explicit headless/completed
  discovery must agree with CLI; `--all` continues to mean all repositories.
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
The latest 2026-09-28 rebase integrates main `1dce02734`; published head is
`fb100f486`. Integration of main `bc51da30` is queued for the next owned
implementation boundary, not applied. It retains PR1317's saved Session executable/Home/database and
launch-failure journal evidence, plus the subsequent landing and schema-cache
changes. Two focused schema-cache reconciliation tests pass. Preserve those
upstream behaviors across Session/Run conversion. Main owns later rebases.
Hosted Rust at `594c7c319f` stopped at the comments fixture's missing Project
status after 930 passes, with 1,013 tests unrun. `0131ed763f` repairs that fixture
and related response builders without weakening required Project fields. Five
focused tests, formatting and all-target Clippy pass on the isolated fixture
tree. New CI36515601936 reaches two completion-fixture failures at duplicate
`projects.external_project_id`, after 932 passes with 1,010 unrun. Their shared
setup and retained-predecessor mock now pass the twelve-test planning module;
`70e8db9c6` publishes that fixture-only repair. Its CI then failed the hierarchy
fixture; `fb100f486` publishes that one-line correction. The latter CI is terminal
at the obsolete Project-status prohibition: 1,304 passed, one failed, 643 unrun.
After its narrow correction and rebase, run one no-fail-fast materialized Rust
matrix to expose remaining failures together; retain exact source and all results.
Full CI is owed. Resumed loop-decide iteration9 completed and chose Iterate;
implement iteration10 is running after operational recovery of the quoted-output
classifier failure. This does not finish
the conversion or discharge any of the remaining requirements above.
Do not cite assertion passes with a leaked process as clean settlement. Review
complete public behavior and deletion paths; measure code by the same method/base.
Update docs, skills and generated HTML with actual behavior. Compress and follow
the saved Flow's review boundaries; code-complete concept review is the requested
stop. Configured Desktop/provider, real-Home upgrade and release acceptance remain
separate explicit obligations with a prepared procedure, not fabricated passes.
