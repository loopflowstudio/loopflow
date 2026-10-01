# Resolve Tasks through a synced local store and use the official worker runtime

Status: implementation follows Jack Heart's 2026-09-30 scope cut below. The
2026-09-29 approval and older installation-continuation plans remain historical
context; they do not reinstate multi-store succession. Main is merged locally;
the waiting sync caller owns final verification and publication.
Review feedback: [repository connection and Task validity](repository-planning-connection-review.md).
Open choices: [questions](questions.md).
Research: [Apollo, Relay, Realm and PowerSync](planning-store-sync-research.md).
Command walkthrough: [Dave takes an idea to running work](idea-to-task-command-story.md).
Wave mapping and remaining migration details: [Wave existence and Linear migration](wave-existence-and-linear-migration.md).

## Main integration — 2026-09-30

Merge target `61d21f885` includes Jack Heart's later LOO-298 compression decision:
retain current state and final Session owners; remove historical import and
intermediate draft-schema compatibility. That decision supersedes the older
migration-preservation sections below. The planning migration now starts at the
released 0.12.29 schema, with three planning drafts. Released SQL remains unchanged.
The old execution drafts, Session importer, branch-history bridge and their
obsolete fixtures are removed. No installed Home is opened by the proof.

Main's single FlowSession driver retains managed-planning validation after native
recovery and completed-Task cleanup, before another boundary starts. Main's Task
lifecycle and sync commands remain intact; adoption keeps this branch's existing
worktree, PR and saved-progress behavior. CLI reference additions follow main's
split between the tour and command reference. Broader gate and configured
acceptance remain with their existing owners.

Verification: materialized `cargo test -p loopflow --lib` filters for `automatic_refresh_reports_failure_with_retained_observation_age`, `migration_preserves_planning_identity_and_removes_snapshot_storage`, and `task_sweep_previews_old_chapters_and_preserves_current_and_terminal_issues` passed (3 tests); `cargo test --test task_adoption_tests --no-run` passed; logs `.lf/tmp/sync-main/`; gate/CI retain broader checks.

The first sweep proof exposed conflicting simulated Project facts between list
and detail. The shared fixture now gives each Project consistent names, statuses
and revisions and advances issue revisions after mutations. Production rejection
of conflicting evidence remains unchanged. Review also moved managed admission
after completed-Task cleanup so successful completion can settle. The adoption
fixture uses `sync --plan` and the final FlowSession shape. The public installation
scenario was compiled, not rerun; no configured-provider acceptance follows.

`lf sync --continue` created merge `e42db82ef`; its cleanliness check reported
related deletions outside the original conflict paths. A local Loopflow cleanup
commit includes those changes. No second sync or push is performed by this Run.

## Current implementation scope — 2026-09-30

Jack Heart superseded installation-copy continuation: one main Home owns work;
custom test Homes are disposable. LOO-342 owns the broader installation/promotion
cut. This Task removes its cross-installation execution discovery and copied-Task
fingerprints instead of extending that superseded contract. Historical receipts
below describe prior attempts, not remaining acceptance requirements.

Finish Task adoption through public `task checkout` and `task run`: consume
Linear's branch name, reuse Git's existing worktree without altering authored
bytes, and retain an observed open PR. Existing Task identity and saved Flow use
the existing continuation path. An open PR or checkout cannot establish a Flow
cursor; no progress is invented from either. Prove adoption from a store with
planning but no Task row, then repeated checkout/continuation preservation.
No source binary touches the installed Home. Integration merges main; no rebase.

Compression removed the receipt-list API left by the superseded succession path:
its only caller now validates and collects retained installations directly,
without an intermediate receipt vector. Removed two leftover temporaries and
placed validity-case expectations beside their inputs. Review corrected stale
discovery and outage-policy prose. Managed-Flow lookup retains its child-pass
semantics; no adoption, admission, migration or installation policy changed.

Verification: 23 isolated `machine_install::tests` passed, including retained
store discovery and immutable receipt settlement (`.lf/tmp/task-compress-installation.log`).
`cargo fmt --check`, all-target Clippy with warnings denied
(`.lf/tmp/task-compress-clippy.log`), and diff checks passed. The public adoption
and due-refresh proofs below remain applicable to unchanged behavior and were
not rerun. Main integration, live-provider and installed acceptance remain open.

### Managed validity — current implementation, 2026-09-30

Saved continuation previously skipped planning lookup whenever a Flow existed.
Continuation/restart now use one managed-planning resolver before execution
mutations; the shared driver repeats it before each boundary of the Task's selected
Flow. It rejects terminal planning and mismatched Project/Wave/Team, retaining
execution history rather than adopting a new plan. A driver stopped by planning
releases its own claim without resetting the cursor or creating a provider-failure
recovery Ask. Independent attributed Flows do not acquire this managed restriction.
Completion evidence from already executing work remains history; this is admission
at boundaries, not distributed exclusion of concurrent provider changes.

Outage policy accepts the existing fresh-cache interval. A due refresh that fails
refuses managed work; inspection still exposes the retained observation and age.
Connection changes select current configuration without copying/publishing old
planning. These choices resolve the corresponding questions under Jack Heart's
headless direction.

Proof and review:

- `uv run python scripts/test_task_installation.py --test task_adopts_linear_checkout_and_preserves_saved_progress`
  passed in `.lf/tmp/task-validity-public-5.log`, including its four populated
  migration prerequisites. All three public adoption cases still pass. The
  existing-worktree case additionally proves canceled, moved, changed-Team and
  removed planning refuse public continuation without changing the Task, saved
  review/graph/cursor or PR. An internal worker entry independently refuses
  removed planning, releases its claim and retains the captured boundary. An
  independent Flow then finishes in the same checkout without advancing the
  managed Flow. Its mechanical `rebase --plan` operation performs no rebase.
- The focused `automatic_refresh_reports_failure_with_retained_observation_age`
  test passed in `.lf/tmp/task-validity-focused.log`: a simulated provider outage
  refuses a due automatic refresh while inspection keeps the original facts/age.
- Formatting, all-target Clippy with two workers, and diff checks passed;
  `.lf/tmp/task-validity-clippy-final.log` retains Clippy. Resource preflight
  passed at 37.6 GiB free; the later observation was 46 GiB. Builds used at most
  four workers total. No full gate ran in this implementation pass.

The first four public attempts remain failed evidence. They exposed a PR
comparison taken before a legitimate observation, a review position correctly
refusing a worker claim, the internal command spelling `__worker`, and an
unsupported mechanical `session list` fixture. The corrected proof preserves
those boundaries instead of weakening production admission. All proof containers
were removed. Planning, GitHub and terminal transport remain simulated; Git and
CLI execution are real. No live Linear mutation or installed acceptance is claimed.

Review removed restart's obsolete Project reassignment and corrected documentation
that had described `task run --flow` as replacing saved progress. The selected
Project identity now survives restart, and replacement remains explicit.

Merge integration is still blocked: installed `lf` help exposes no merge command
under the root, `wt`, or `rebase`. The last-fetched `origin/main` at `e14a1d035`
has eleven commits absent from this branch. No fetch, merge, rebase, publication,
Task completion or Flow navigation was performed in this pass. The caller retains
integration and delivery; a passing isolated proof does not establish either.

### Adoption proof and review — 2026-09-30

Implemented Linear `branchName` in shared list/detail planning and the wire model.
Task preparation reuses the existing Git placement strategies, retains an observed
open PR, and fetches its branch when a fresh clone has not seen it. Its base is
the existing branch's merge base with the selected base, not a new branch tip.
Stack parent selection still uses `--stack-on`; no parent Task is inferred from
a PR title. Checkout preserves dirty state; run retains normal Flow behavior,
including checkpointing when entering review. Existing Task Flows retain their
captured graph, node, pass, review reservation and version.

Removed copied-Task fingerprints, receipt copy fields, cross-installation Task
and Session discovery/forwarding, two superseded succession proofs, and their
unused release-provenance fixture build. LOO-342 retains the wider installation
simplification. No source SQL migration or installed Home was changed.

Review found and fixed three integration assumptions: initial Task insertion
required an unpublished PR rather than an active PR; equal-revision planning
could not enrich a previously unknown branch name; and Task-status fixtures and
Swift still carried the retired Project `flows` shape. Existing ownership,
terminal-state and conflicting-known-revision checks remain in their owners.

Proof:

- `uv run python scripts/test_task_installation.py --test task_adopts_linear_checkout_and_preserves_saved_progress`
  passed in `.lf/tmp/task-adoption-public-5.log`: three real CLI/Git scenarios,
  same-revision branch enrichment, checkout bytes/identity and open PR retained,
  public run adoption, remote branch recovery, and exact saved later Flow/review
  preservation after catalog edits. The harness also passed its four populated
  migration prerequisites. Planning/GitHub/terminal transport are fixtures;
  no live provider turn, published release or installed acceptance is claimed.
- `.lf/tmp/task-adoption-focused-2.log` records the passing shared Linear
  lookup/cache proof. `.lf/tmp/task-adoption-focused-3.log` records passing Rust
  Task-status wire and dirty-checkout recovery proofs and the Swift
  `DTOFixtureTests/taskPlanningBranchFixture` check (one Swift Testing test).
- `cargo fmt --all -- --check`, `cargo clippy --all-targets -j 2 -- -D warnings`,
  Ruff and `git diff --check` passed. Final Clippy log:
  `.lf/tmp/task-adoption-clippy-final.log`. No full gate ran in implement.

Earlier failures are retained: public proof 1 had a GitHub stub rejecting
`--version`; proof 2 exposed the production initial-PR restriction; proof 3
lacked stable review IDs; proof 4 reached review preparation without tmux.
The final proof supplies a simulated terminal and does not reinterpret those
failures as successful provider execution. The initial local lookup and DTO
checks exposed stale Project status/Flow fixtures; corrected checks passed.
Every proof container was removed. Resource preflight initially passed at
46.3 GiB and the final observed free space was 41 GiB; builds used two workers
per independent check and nice +10.

Main was not rebased or merged. The supplied installed `lf` has no merge command
or merge option; the requested merge-only integration remains with the caller.
Publication, Flow navigation and Task completion were not performed.

Compression removed the deferred-future wrapper around stack-parent lookup,
reused the placement plan's resolved default branch, and consolidated worktree
creation's repeated path checks and result construction. Review found that the
adoption change also removed the occupied-path preparation check: an unrelated
directory could let issue creation succeed before checkout failed. The existing
regression reproduced this in `.lf/tmp/task-adoption-compress-focused.log`.
Preparation now refuses that collision before filing; actual Git worktrees still
adopt, and execution rechecks the destination after provider work.

Final compression proof: five Task-preparation tests and the worktree creation/
reuse test passed with all-target Clippy in
`.lf/tmp/task-adoption-compress-final.log`. The public adoption proof passed its
three scenarios and four populated migration prerequisites again after the fix
in `.lf/tmp/task-adoption-compress-public-final.log`; its container was removed.
Formatting and diff checks passed. These retain the fixture limits above; no
installed or live-provider acceptance is established. Resource preflight passed
at 41.2 GiB free; the later sample remained above the 32 GiB reserve at 39 GiB.

## Implementation checkpoint — 2026-09-29

### Compression blocked by disk reserve — 2026-09-30

At `230e3333e`, compression inspection made no source changes. Resource preflight
reported 31.2 GiB free against the required 32 GiB floor. The prescribed
`uv run python scripts/resource_envelope.py --recover` finished with 30.4 GiB
free and no reclaimed space; active and recent builds were retained. A subsequent
`UV_LOCK_TIMEOUT=0 uv cache prune` also failed because another process held the
cache lock. No behavioral checks ran, and earlier proof is not a new pass.
Resume compression after resource preflight permits testing. The untracked
runtime/store decision consultation remains untouched and pending.

### Resumed implementation — official step selection, 2026-09-30

At `1fd99a284`, the supplied worktree was clean. Jack Heart's latest direction
keeps this branch stacked and forbids rebasing until LOO-298 republishes its
compressed branch. No rebase, publication, host promotion or host-store experiment
was performed. Resource preflight passed with 44.0 GiB available against the
32 GiB floor; the disposable build used two workers at nice +10.

The public retained-installation proof exposed a production defect: the resumed
Task worker selected the obsolete `lf` first on PATH for its mechanical child.
`resolve_step_lf_binary` now reuses the verified official-runtime selector before
considering PATH. Artifact verification failure remains an error. Uninstalled
machines retain PATH, then the current driver, as fallbacks. This follows Jack's
approved rule that ambient PATH is not a deliberate pin. Self-review removed the
duplicate installation lookup instead of adding another selector or runtime owner.
Persisted explicit locks and provider-shell propagation remain unfinished.

The fixture now supplies terminal transport before launch, observes a published
review, runs an actual mechanical step on each side of that review, and checks
both worker Execs. Historical conversation setup imports the captured-input files
through public `session import`. The incompatible-draft fixture works after
canonical materialization; divergent execution uses a fresh invocation and compares
the complete persisted Flow before and after refusal. The harness accepts several
names after `--test` to share one disposable build across related proofs.

**Focused evidence:**

- `loo334-official-step-proof.log`: four populated migration checks and public
  `flow_step_executable_falls_back_without_losing_its_store` passed. This covers
  uninstalled driver/PATH fallback and installed precedence over PATH. The later
  test failed on a fixture worker-count expectation; this is not a green combined run.
- `loo334-two-worker-final.log`: four populated migration checks and public
  `installation_switch_preserves_task_review_without_store_overrides` passed.
  Public status/run/open/ready/complete retain the exact published review and
  original execution directory, execute two Task workers under their respective
  installation binaries, complete the Flow once, preserve incompatible database
  bytes, import/reopen retained conversation history with PTY input, and refuse
  divergent execution without changing either Flow. Installation records and
  terminal/provider effects are fixtures, not normal promotion or configured
  provider acceptance. The fixture's review launch reported missing Claude.
- `cargo fmt --all`, all-target Clippy with warnings denied, Ruff on the harness,
  and `git diff --check` passed. Logs are under `.lf/tmp/`. Every disposable
  container was removed and its absence independently confirmed.

**Retained failures:** `loo334-resume-installation-switch.log` found late tmux
fixture setup; `-2.log` observed `review Run changed while opening` during
publication; `-3.log` found the absent synthetic draft ledger; `-4.log` exposed
the obsolete PATH child. `loo334-official-step-proof.log` then found the fixture
started at review and had only one Task worker. `loo334-two-worker-proof.log`
passed both workers but found the old prefixed-artifact directory assumption;
`loo334-two-worker-proof-2.log` passed terminal input and divergent refusal but
compared input version 0 against persisted version 1. The final proof excludes
no fields from preservation. Waiting for publication before opening establishes
stable-review continuation only; the publication-concurrent Open failure remains
unfixed and is not rewritten as a pass.

**Remaining boundaries:**

- The ordinary installed `lf ask` opened
  `ask_883713ce782642b8adf9b73e07d4daa5`; its waiting command has not returned a
  decision. The original production-directory/selected-development-runtime
  conflict remains. The Session owns `installation-runtime-store-decision.md`;
  its recommendation is not approval. The relationship-repair Ask is separate.
- Delayed startup needs the actual child's Exec/process evidence before the
  caller returns Starting. Removing the ten-second claim release alone is
  insufficient: the claim still names the launching Exec, whose later exit lets
  a retry reclaim before the delayed worker's handoff. Preserve the existing
  version/generation fence; a tmux pane is not a synthetic lf Exec.
- Chapter archival must recover beyond a lost completion response. The stacked
  operation never calls `complete_and_archive_project`; simply adding it misses
  retry because `plan_rotation` selects no predecessor once the successor is
  Started and the old Project Completed. Identify the intended historical Project
  through provider facts, without silently choosing newest history or archiving
  every Completed Project. These startup/archival findings are source inspections,
  not new behavioral proofs.

Normal promotion's production-store failure, explicit locks, provider-shell PATH,
Session-owner discovery and full acceptance cases 1–15 remain open. These focused
results establish neither a full gate nor whole-design completion.

### Slice review — recorded installation copies, 2026-09-30

**Blocked; no publication.** Reviewed the complete slice from `2d63d6488` to
`9fea0552f`, including its consumers, receipt transitions, tests and documentation.
The required public proof still fails before ordinary Task continuation. Resolving
that failure changes the runtime/store contract, beyond a bounded review repair.

Fresh proof: `uv run python scripts/test_task_installation.py` passed **3 + 1
populated migration checks**, built both CLI provenances and compiled the selected
integration targets. The first public case preserved first-install recovery,
Task-owned refusal, the first worker, exact review publication and both complete
Flow comparisons across normal promotion. Public status then failed at
`task_initialization_tests.rs:661`: the selected development CLI refuses
`/home/lf-task-proof/.lf/loopflow.db`. The accompanying missing Exec-ledger message
does not establish a different cause. Log:
`.lf/tmp/loo334-review-copy-installation.log`. The remaining nine public cases,
review completion, second worker and divergent-copy refusal were not reached.
The harness removed its disposable container; an independent Docker listing
confirmed absence. No host installation or Home was mounted.

The source review finds real consumer replacement: recorded backup provenance
replaces treating every matching installation as independent execution, and
AgentSession/captured-event lookup replaces manifest scanning for retained Session
discovery. Unknown or changed copies still refuse routing; readiness forwards its
exact token to the owning store. Final production Rust delta is **+239 / −34**
(net +205), excluding trailing unit-test modules, integration tests, documentation,
scratch and inherited parent work. The preceding receipt-pinned slice also replaced
real consumers; the two successive no-replacement rule does not apply. Reuse the
recorded formatting/Clippy passes for these unchanged source bytes.

Next: reconcile selected development runtime, original execution-directory
preservation and production-store isolation, then repeat this same proof through
automatic continuation, exact review completion and the second actual worker.
Do not silently select the retained release, transfer ownership to the copy or
weaken isolation. The failed policy Ask remains failed evidence; do not duplicate
or modify the separate pending relationship-repair Ask. Recursive locks, provider
shell PATH, independent Session discovery, chapter archive integration and
acceptance cases 1–15 remain open. This review establishes neither configured
acceptance, host promotion, Task completion nor whole-design acceptance.

### Current slice — recorded installation copies, 2026-09-30

Incoming direction: preserve normal promotion's complete Flow copies while making
ordinary status/run/review continuation find their original execution. Keep the
selected runtime independent of the store. Genuine independent copies remain
conflicts; neither equal IDs, timestamps nor selected-store preference settle them.

The existing SwitchReceipt now records the actual backup source and per-Task
fingerprints of copied execution after migration, before activation. Source
identity is fixed at TargetPrepared; the baseline is fixed at Advancing and
survives recovery. Prior selection alone was insufficient: fresh development
promotion copies the reliable database even when `prior` names a development
store. No database table or execution lifecycle is added. Old receipts omit the
new optional fields on reserialization, preserving immutable retry bytes; they
provide no copy evidence and cannot silently authorize routing.

Discovery compares the backup with its baseline, retaining the original execution
owner while it progresses. A changed copy, missing source or unknown baseline
stays unresolved. Task, PR/delivery, FlowSession, AgentSession, linked Exec and
subordinate history rows contribute to the comparison. New schema columns change
the comparison; this cut does not guess across subsequent schema changes or
rebaseline an exposed copy. Taskless conversation-copy succession remains open.
Session discovery reads AgentSession and captured-event identity rather than
searching manifest files. Forwarded readiness retains its exact capture/token and
uses the owning store's existing check; it does not advance the Flow.

The normal-promotion fixture now expects the original execution directory to
progress and the copied Flow to remain untouched, matching acceptance cases 6
and 15. It poisons PATH, LF_BIN and LF_CONTROL_BIN with the predecessor, invokes
public status/run/open/ready/complete, requires two worker command identities and
one Flow completion, then changes the copied review feedback and requires a
conflict. Terminal transport remains simulated. The old expectation that the
copy should progress is superseded; both pre-continuation Flow comparisons stay.

First focused run: four populated migration checks passed. Promotion then failed
on the baseline reader's obsolete `steers` query; current Steers are already in
Task events. Removed that query rather than treating a missing table as empty.
Log: `.lf/tmp/loo334-copy-continuation.log`.

Second focused run: all four migration checks passed; published first-install
advance/recovery, Task-owned recovery/promotion refusal, the first worker,
review publication, local promotion and both full Flow comparisons passed.
Public status then reached the resolved original store and failed with
`development lf (...) refuses production database /home/lf-task-proof/.lf/loopflow.db;
use an installed release lf`. Log: `.lf/tmp/loo334-copy-continuation-2.log`.
The failure is `store::guard_development_database`, before the owning Task can
be read. The new conflict is between selecting development bytes and preserving
the published execution directory. Later continuation/divergence assertions were
not reached. Do not call the fixture green or relax the production-data boundary.

Dependent implementation stops for contract review. Using the retained release
changes the selected-runtime requirement; transferring execution into the copy
changes the preserved-directory/no-transfer requirement and requires a real
ownership handoff; permitting selected development bytes to write the production
store changes installation isolation. None is silently selected here. The
receipt/lookup change remains reviewable but does not complete succession.

The required durable decision handoff was attempted with `lf ask`, describing
all three policy alternatives and requesting no host action. It failed before
opening a Session: `Error: Task "LOO-334" is not registered`. No new Ask was
created, no attribution/store overrides were used, and the existing separate
relationship-repair Ask was not modified. Return this concrete conflict to the
caller; do not repair the registration or weaken isolation to reach the review.

Production Rust against `2d63d6488`: **+244 / −32 lines** (net +212), including
the new copy-evidence reader, excluding tests/test-helper fields, docs and
scratch. No source migration changes.

Final source checks:

- `uv run python scripts/test_task_installation.py`: **failed** at the same
  production-database refusal after **3 + 1 populated migration passes**, both
  CLI builds and all selected integration-target compilations. The first public
  case reaches the original store after preserving both full Flow comparisons;
  the remaining nine public cases and all later continuation assertions do not
  run. Log: `.lf/tmp/loo334-copy-required.log`. The disposable container was
  removed by the harness. No host installation/data was mounted.
- `cargo fmt --check`: passed. `CARGO_BUILD_JOBS=2 nice -n 10 cargo clippy
  --all-targets -- -D warnings`: passed; log
  `.lf/tmp/loo334-copy-clippy-final.log`. `git diff --check`: passed.

These checks do not establish review completion, a second worker, divergence
refusal, configured providers or installed acceptance. The whole design's
acceptance cases 1–15 remain open; this is a failed behavioral proof with a
concrete policy counterexample, not a gate or delivery result.

Review also caught two preservation details: a lone backup cannot inherit
ownership when its recorded source is missing, and optional receipt fields must
not add null bytes when retrying an immutable older receipt. Both are addressed.
The copy fingerprint is local recorded evidence, not a distributed write lock;
concurrent independent writes can still make a later lookup ambiguous.

No change to the pending relationship-repair Ask. Recursive locks and provider
shell PATH, independent Session discovery, chapter archive integration and the
full acceptance matrix remain open. No publication, host promotion, Task
completion or whole-design acceptance follows from this slice.

Compression removed copy-index bookkeeping, the repeated baseline lookup and a
Task-ID clone (net −7 production lines), preserving comparisons and error paths.
Formatting, all-target Clippy and diff checks passed; Clippy log:
`.lf/tmp/loo334-copy-compress-clippy.log`. Behavior is unchanged; the failed public
proof above was not rerun and its policy counterexample remains unresolved.

### Earlier slice — receipt-pinned installation continuation, 2026-09-30

The incoming migration review is preserved at `3b4cd6592`. This cut removes the
ordinary-selection dependency from `advance_switch`; the initiating promotion
still checks Task ownership, and advancement still verifies the exclusive
coordinator lock, exact receipt, candidate path/digest, handoff phase and build
authority. Direct recovery remains a separate caller: it checks Task ownership
read-only in the receipt's advanced target, or its prior store before advancement.
It never opens the ordinary selected registry for that check. As with ordinary
ownership resolution, an absent database is unmanaged only without a Work
declaration; inaccessible evidence still fails. No receipt or store schema changes.

The disposable proof now interrupts first activation after target-store advance,
registers a Task, refuses recovery from that Task checkout, and recovers with the
receipt's candidate from an unmanaged Git checkout. First installation remains
inside Git; moving it outside Git is not the fix. The later development promotion
also checks Task-owned refusal before using the unmanaged Git checkout.

The first harness pass reached Task admission after successful first-install
advance/recovery, then failed because control-context resolution did not recognize
the installed `lf-<digest>` executable without PATH assistance. Installed pinned
execution now resolves and verifies its executable through the existing receipt;
uninstalled resolution keeps its existing behavior. No basename allowlist or
ambient PATH repair was added.

Review caught that simply removing the recovery guard would permit Task-owned
recovery. The final implementation retains it with an explicit database input to
the existing ownership check. The second build caught a `Store`/`SqliteStore`
constructor mismatch; it was corrected before behavioral verification. The third
harness pass confirmed Task recovery refusal and unmanaged recovery, then retained
the same admission failure; its source snapshot predates the executable fix.
Logs: `.lf/tmp/loo334-receipt-installation.log`, `-2.log`, and `-3.log`.

The final-source attempt in `.lf/tmp/loo334-receipt-installation-final.log`
stalled during compilation while Docker also failed a bounded ten-second info
probe. Its owned harness was interrupted; exact container cleanup subsequently
succeeded after Docker responded. The retry (`-retry.log`) passed the four
migration checks, first-install recovery, Task recovery refusal, the first real
worker and pending review preparation. It then disproved the existing promotion
guard: the development candidate's earlier private snapshot lacked the Task,
so promotion from the Task checkout succeeded in the disposable container.
Promotion now checks the selected installation's store as well as its ordinary
context through the same ownership resolver. No host installation was changed.

The ownership rerun (`-ownership.log`) passed Task-owned promotion refusal and
promotion from the unmanaged Git checkout, then failed full Flow preservation.
The initial hypothesis was a stale fixture baseline from before `session open`.
The fixture now captures the full state after `session open`, before the two
promotion commands, and still compares both old and copied stores against it.
The repeated failure below disproves that baseline-only explanation.

Final behavioral command: `uv run python scripts/test_task_installation.py`, log
`.lf/tmp/loo334-receipt-installation-continuity.log`. All 43 drafts materialized,
the four populated migration tests passed, and both executable variants built.
The normal-promotion scenario passed first-install advancement in Git, failed
activation recovery, Task-owned recovery refusal, the first actual worker,
pending-review preparation, Task-owned promotion refusal, and unmanaged
development promotion. It then failed at the copied Flow preservation assertion:
the pre-promotion baseline has `version=2` and `published=false`; the copied
Flow has `version=3` and `published=true`. A comparison of the full assertion
payloads found only those two differences. This does not yet locate the writer:
the refused source invocation, successful promotion, store opening and copy
inspection are all between the observations. Do not call this a harmless
projection, move the baseline again, or weaken the assertion without finding it.
The public post-switch status/open, exact completion, second worker and digest
assertions were not reached; later harness tests did not execute.

Next isolate when that Flow mutation occurs and reconcile it with the preserved
review contract before proceeding into normal-copy succession. Keep old-store
and copied-store evidence distinct. The final fixture retains the full comparison
and the required two-worker continuation path. The implement skill's counterexample
rule returns this unresolved preservation finding to review; it does not choose
Flow navigation or authorize a new installation policy.

`cargo fmt -- --check`, `cargo clippy --all-targets -- -D warnings` and
`git diff --check` passed. Final Clippy log:
`.lf/tmp/loo334-receipt-clippy-continuity.log`. The last edit only corrects the
fixture comment and this evidence note. Against `3b4cd6592`, production Rust is
**+47 / −8 physical lines** across three files, excluding integration tests,
docs and scratch. No runtime owner, migration, compatibility alias or Task
disposition was added. The earlier mutation-free local Task guard assumption
is no longer sufficient evidence for promotion preservation.

Compression shares current/sibling executable discovery without changing lookup
precedence or receipt checks (22 fewer production lines). All ten process tests,
formatting and all-target Clippy passed; log: `.lf/tmp/loo334-compress-process.log`.
The installation harness was not rerun; its recorded Flow-state mismatch remains open.

Full-design cases 1–15 remain the publication boundary. Normal-copy
succession, exact review completion and two worker digests, genuine divergence,
recursive locking and decoy agent PATH, Session-owner discovery, chapter archive
integration and the separate pending relationship Ask remain open. No publication,
host promotion, Task completion or Flow navigation follows from this cut.

### Slice review — receipt-pinned continuation, 2026-09-30

Reviewed `3b4cd6592..5a2e54b79` and the bounded fixture correction below.
The installation continuation reuses the receipt, exclusive coordinator lock and
exact candidate verification. Recovery checks its caller against the receipt's
database read-only; ordinary promotion also checks the selected store. This fixes
the first-install selection cycle without deleting Task-owned refusal. Production
Rust is **+57 / −40 physical lines** across three files (net +17), excluding
tests, docs, scratch and inherited parent work. Shared executable fallback removes
the duplicated current/sibling lookup; no runtime owner or migration is added.

The required full harness at `5a2e54b79` reproduced the copied-Flow mismatch after
all four migration checks and both executable builds passed. Log:
`.lf/tmp/loo334-review-receipt-installation.log`. Resource preflight passed at
47.4 GiB free. The writer is `publish_review_run`, called by the separately
launched review skill: its transaction increments the Flow version and publishes
the selected Session input. Parking the worker and `session open --json` do not
wait for that child. The fixture now waits for publication before promotion and
checks that the selected review, captured event and cursor remain unchanged;
both full copied-store and predecessor-store comparisons remain in place.

The focused rerun,
`uv run python scripts/test_task_installation.py --test normal_promotion_preserves_pending_task_review`,
logged **version 2 → 3, published false → true, captured 1 before either promotion
command**. Both full Flow preservation assertions then passed. This resolves the
prior mismatch as concurrent review publication, not a demonstrated copy mutation.
First-install advance/recovery inside Git, both Task-owned refusals, the first
worker and unmanaged development promotion also passed. Log:
`.lf/tmp/loo334-review-published-review.log`.

**Continuation remains blocked.** The next public `task status INF-123 --json`
failed with `execution exists in multiple distinct locations`, naming the ordinary
promotion copy and its retained predecessor. `task_destination::existing_execution`
rejects every pair of distinct matching stores; it cannot recognize this supported
installation succession. Next implementation must resolve normal-copy succession
from retained installation/execution evidence while keeping genuine divergence
explicit. Neither equal Home IDs, timestamps nor an unconditional preference for
the selected store establishes ownership. Do not weaken that refusal globally.

Exact post-switch review continuation/completion and the second worker/digest
assertions were not reached. The normal-promotion fixture still lacks poisoned
PATH/LF_BIN/LF_CONTROL_BIN; the separate fixture cannot substitute for that combined
proof. Recursive locks and decoy agent PATH, Session-owner discovery, chapter
archive integration, the pending relationship-repair Ask and full cases 1–15 stay
open. This required failing proof stops the slice review; no publication or Flow
navigation follows. No host installation was touched. Both proof containers were
independently confirmed absent after cleanup. Fresh formatting, all-target Clippy
(`.lf/tmp/loo334-review-receipt-clippy.log`) and diff checks passed. The review changes
only fixture synchronization and this section; it adds no production behavior.

### Slice review — migration integration, 2026-09-30

Reviewed `08e70e45c..d6f71dcc2` against Jack Heart's completed migration unblock.
The migration cut is supported; installation continuity is still blocked.
Fresh command: `uv run python scripts/test_task_installation.py`, log
`.lf/tmp/loo334-review-migration-installation.log`. Resource preflight passed at
35.6 GiB free. In disposable Docker, all 43 drafts materialized, the three
populated release/branch-history and rollback tests passed, and the normalized
planning preservation test passed. Both executable variants built. Applied
checksums remain unchanged and temporary snapshot inputs leave no runtime owner.

The first installation test, `normal_promotion_preserves_pending_task_review`,
failed with `Task PR authority refused: the shared Loopflow registry path is not
usable: first installation is unfinished`, followed by `install switch candidate
exited exit status: 1`. The latter diagnostic identifies **advance-switch**, not
a demonstrated recovery attempt. Source explains the cycle: `run_switch_candidate`
inherits the Git checkout; `advance_switch` calls `guard_task_checkout` before
receipt validation; Task ownership resolution requests ordinary installation
selection, which has no prior selection during first install. `recover_switch`
has the same early guard. Its failure is a source finding, not a separately run
recovery proof. The previous note's “recovery child” description was imprecise.

The supplied review-slice instruction says an unrunnable required proof is a stop.
Return this implementation gap to the next cut: let the receipt-pinned machine
operation validate its own authority without requiring ordinary startup selection,
while preserving the prohibition on Task-owned promotion. Do not move the fixture
outside Git to hide the failure. Then rerun the harness through exact review
completion and both real worker boundaries with selected-artifact digest evidence.
No promotion or worker proof passed; later harness cases did not execute. The
proof container was independently confirmed absent after cleanup. No installed
Home was accessed and no production code changed during review.

Measurement against `08e70e45c`: **+130 / −29 production Rust lines**, excluding
test modules/helpers, integration tests, scripts, docs and inherited parent work;
three new SQL files add **62 lines**, with two existing dependency-header changes.
The 40 historical receipts are import data. This repairs the shared migration
owner rather than adding another runtime store. Existing static-check results
below remain prior evidence; this review adds the disposable harness result.

Full-design cases 1–15 remain the publication boundary. Normal promotion's new
two-worker assertions do not include the decoy PATH setup; the separate poisoned
runtime fixture cannot establish recursive agent PATH correctness. Session-owner
discovery, divergent installation copies, chapter completion/archive integration
and the independent pending relationship decision remain open. No publication,
Task completion or Flow navigation is justified by this review.


### Current slice — preserving migration integration, 2026-09-30

Jack Heart's completed unblock is implemented for the migration boundary. This
supersedes the earlier documentation-only preflight as current implementation
status; the failed observations below remain evidence. The full design, cases
1–15, the decoy-PATH regression and the independent relationship decision remain
open. No publication, host promotion, Task completion or Flow verdict follows.

The causal conflict is verified: normalization drops `pm_snapshots` before chapter
conversion reads it; chapter conversion first would discard the archive receipts
consumed by `pm_project_evidence`. Three forward drafts preserve the reader inputs
inside the existing migration transaction, convert current and archived Project
bodies, and remove the temporary inputs. Runtime still has one normalized planning
owner. Applied SQL bodies and checksums are unchanged; only two dependency headers
change the order for new stores.

The ordinary migrator now recognizes the exact ordered receipts from `b1719ea6c`
(three planning drafts) and `b21f657fc` (37 execution drafts). It verifies the
historical schema, executes missing SQL, and checks the final common schema. Draft
append preserves existing receipt IDs, timestamps and checksums; canonical adoption
consumes the same unchanged bodies. Unknown IDs, changed checksums, reordered
history and schema drift remain rejected. This is a bounded import of recorded
histories, not a schema-equality shortcut or another upgrade command.

The populated fixture starts at released `0.12.24` and runs the ordinary full
migration chain from that schema and both original draft histories. It retains
Task/Project/PR identity, worktree, exact Flow/captured-input identity, review
feedback, cursor/version and Started evidence. Archive receipts retain their
observation time and custom Flow. The execution-only history had already removed
its predecessor receipts; the proof explicitly rejects inventing an archive fact
for that missing evidence. A failing subsequent draft rolls back schema, all
retained rows and both ledgers for all three origins.

The full-chain consumer fixture exposed a second counterexample: delayed Project
list input failed with `unordered or conflicting Project facts for retired-project`
after archived bodies correctly gained Completed status. `put_project` now retains
confirmed archive facts when later list/detail data arrives. The existing delayed
list/removal test passes through the full chain. The first rollback probe used a
nullable Project slug and did not fail; the replacement injects a failing next
draft after integration and proves complete rollback instead. Neither observation
has been counted as a pass.

**Verified locally, without installed-Home access:**

- Source: `cargo test -p loopflow --lib planning_integration -- --nocapture`
  **3 passed**; `cargo test -p loopflow --lib
  migration_preserves_planning_identity_and_removes_snapshot_storage` **1 passed**.
- Disposable exact-source copy at `.lf/tmp/loo334-materialized`, materialized with
  `scripts/canonicalize_migrations.py 0.12.25 --materialize-for-tests`: the same
  **3 + 1 passed**, plus `cargo test -p loopflow --lib installed_development_
  -- --nocapture` **5 passed**. Canonical construction no longer fails on
  `pm_snapshots`. These are local Rust/SQLite proofs, not installation acceptance.
- `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`,
  `uv run ruff check scripts/test_task_installation.py`, `git diff --check` and
  `uv run python scripts/check_migrations.py` passed. The migration checker reports
  all **55 shipped migrations unchanged**.
- Logs: `.lf/tmp/loo334-migration-source.log`,
  `loo334-planning-fixture-source.log`, `loo334-canonical-planning_integration.log`,
  `loo334-canonical-migration_preserves_planning_identity_and_removes_snapshot_storage.log`,
  `loo334-canonical-installed_development_.log`, `loo334-clippy.log`, and
  `loo334-materialize.log`, all under `.lf/tmp/`.

**Required proof blocked:** the earlier Docker timeout remains recorded in
`.lf/tmp/loo334-installation-migration.log`. The compression retry reached Docker
29.4.0, materialized all 43 drafts, passed the three populated migration tests and
the normalized planning test, and built both executable variants. It then failed
at the first published promotion in `normal_promotion_preserves_pending_task_review`:
`Task PR authority refused: the shared Loopflow registry path is not usable:
first installation is unfinished`. Preflight permitted promotion, but its recovery
child exited 1. Log: `.lf/tmp/loo334-compress-installation.log`; container cleanup
completed. No successful promotion, exact review completion, two-worker or selected
runtime digest proof follows; later harness cases did not run. Those assertions
remain authored. Terminal transport is simulated, so even a future pass does not
prove configured provider or Desktop acceptance.

**Review and measurement:** relative to incoming-notes checkpoint `08e70e45c`,
**+142 / −29 production Rust lines**, excluding tests/test helpers, scripts,
docs and inherited parent changes. Three new migration bodies/files add 62 lines;
two existing dependency headers change, and 40 immutable historical receipts are
retained as import data. The shared migration owner replaces the failed combined
prefix traversal for these known histories. No parallel runtime store is added.
Review caught and fixed the archived-body conversion/late-list interaction; the
import's explicit retirement boundary is documented in `store/MIGRATIONS.md`.

Compression resolves recorded migration names once to candidate positions for both
draft append and canonical adoption, and returns the original unknown-prefix error
without validating twice (12 fewer production Rust lines). Three source and eight
canonical tests, Clippy, formatting and the shipped-migration check passed; logs
are `.lf/tmp/loo334-compress-*`. The first canonical attempt failed two tests because
the shared Cargo target retained the source build's generated schema; rebuilding
the disposable copy's build script regenerated normalized tables and passed.

Next resolve the first-install recovery child's Task-authority failure, rerun the
required harness, and establish case 15 through both worker boundaries. Session-owner
discovery, genuine divergent-copy handling, chapter completion/archive integration,
recursive runtime/decoy PATH and the remaining full-design obligations are not
settled by these migration checks. Keep the pending relationship decision separate.

### Decision protocol and agent PATH defect — 2026-09-30

Jack Heart's steer `8b3c44a1-7a68-4e4d-8c11-88f5dc60f303` adds a required
runtime regression: an earlier decoy `lf` on agent PATH must not override the
selected official or explicitly locked executable. Loopflow-managed PATH
directories must expose that selected executable as `lf`; installation must not
leave an unrelated bare `lf` in `~/.lf/bin`. Workers must diagnose a PATH/runtime
mismatch. Carry this through the existing recursive-lock implementation and
Codex/Claude/OpenCode shell proofs, without another runtime-selection owner.

This decision occurrence independently reproduced the stale command path:
`command -v lf` returned `/Users/jack/.lf/bin/lf`, and the required
`lf flow blocked` returned `step or flow not found: flow`. The installed
`/Users/jack/.local/bin/lf flow blocked` then returned `no current Flow decision`.
The supplied environment names the retained development Home but contains no
Flow/claim/caller authority variables; `LF_BIN` and `LF_CONTROL_BIN` still name
the stale bare path. No authority was reconstructed or transferred. No blocked
Ask or navigation decision was successfully recorded by these commands.

The assessment is Blocked: the intended migration repair was not implemented,
and after disk recovery the required harness repeats the known missing-table
failure. The retained preflight sections below supply before/after evidence.
The controlling caller must restore this exact occurrence's decision protocol
and resolve the stalled repair approach before reassessment. This note is
evidence, not navigation authority. The separate relationship Ask remains
unchanged; no duplicate Ask, publication, promotion or Task completion occurred.

### Slice review — migration preflight, 2026-09-30

**Blocked; no publication.** Reviewed the complete current-slice diff from
`a7db5b03c` through `a1ac71943` and the incoming working note, against Jack Heart's
directive and acceptance cases 1–15. This range changes only this document.
The preserving migration integration has not been implemented.

- **Resource blocker cleared:** fresh `uv run python scripts/resource_envelope.py`
  passed at 39.6 GiB free, above the 32 GiB reserve. The earlier 31.9 GiB refusal
  remains historical evidence; it is no longer the current stop.
- **Required proof failed:** `uv run python scripts/test_task_installation.py`
  exited 1; its release build exited 101 at `rust/loopflow/build.rs:95` with
  `build canonical schema at 0.12.25.001_release: no such table: pm_snapshots`.
  No promotion or worker test ran. Log:
  `.lf/tmp/loo334-review-installation-20260930.log`. The harness removed container
  `cc04c2143944a6f0221f626f4780e1901a09429d117f7fba6ab9d4bd5e6b94aa`;
  a separate bounded all-container query confirmed it absent.
- **Source findings retained:** normalization drops the snapshot input still used
  by chapter conversion; moving chapter conversion first loses the archive input
  read by `pm_project_evidence`. Development prefix/schema validation and canonical
  adoption both need preserving integration. No applied SQL/checksum changed.
  Session location still calls `resolve_manifest`; chapter rotation still omits
  the completion/archive adapter; normal-promotion proof still ends before review
  completion and the second worker. These require implementation, not fresh policy.

**Measurement:** `a7db5b03c` → reviewed working tree: **+0 / −0 non-test
production lines**, excluding docs/scratch, tests, scripts, generated files and
inherited parent changes. No consumer or predecessor path is replaced in this
pass. The preceding implementation (`a9d7ec2fd` → `a7db5b03c`) replaced chapter's
planning reader (+63/−114 production Rust lines, previously measured); therefore
this is not two consecutive implementation passes replacing nothing.

Next implement and prove the preserving migration integration from released
history and both populated branch draft frontiers, including unchanged evidence
and transactional rollback. Then rerun the harness through normal promotion,
exact review completion and two workers with selected-runtime digest assertions.
Retain divergent-copy, obsolete-executable and PTY cases, Session-owner discovery,
archive retry/history and all cases 1–15. The existing relationship Ask is unchanged.
This review changes no executable code, installed data, PR, Task or Flow state.
The required-proof stop returns to loop-decide; it supplies no navigation verdict.

### This slice — migration preservation preflight, 2026-09-30 (blocked)

Preserved the incoming stacked-migration review in `a1ac71943` through the
installed mechanical `lf commit`. No production or migration edits follow that
checkpoint. The ambient `lf commit` first resolved as a skill and failed before
committing; `/Users/jack/.local/bin/lf commit` performed the checkpoint.

**Required verification cannot start:** `uv run python scripts/resource_envelope.py`
exited 1 with **31.9 GiB free against the 32.0 GiB emergency reserve**.
`uv run python scripts/resource_envelope.py --recover` also exited 1 at 31.9 GiB;
its uv cleanup reclaimed zero bytes because the cache lock was held. The prescribed
standalone `UV_LOCK_TIMEOUT=0 uv cache prune` then exited 2 with the same busy-cache
error. No force override, active-build deletion, shared-service restart or installed
data mutation was attempted. TESTING.md prohibits verification below this reserve,
so neither Rust proofs nor `uv run python scripts/test_task_installation.py` ran
in this pass. Docker availability was not retested; the prior SQL failure remains
the last installation-harness result.

**Source observations for the repair:** both draft manifest readers strip
`name`, `id` and `depends_on` headers before computing SQL checksums. Ordering
metadata and applied SQL identity therefore have different contracts, but adding
an ordering edge alone still cannot resolve this conflict. The development
migrator validates both the exact receipt prefix and its reconstructed product
schema; canonical adoption consumes matching draft bodies in order. Any repair
must cover both paths, including their shared rollback transaction, rather than
loosening only one prefix check. The pre-stack LOO-334 and LOO-298 frontiers both
end at canonical `0.12.24.001_release`; their subsequent draft histories differ.

The next bounded discriminator remains a populated migration proof from each
original branch history and from released history. Preserve the original SQL,
receipt IDs/checksums and timestamps. A possible integration must provide the
snapshot and archival inputs before their old readers execute, remove transient
inputs afterward, and explicitly account for already-applied readers on either
frontier. This is an implementation hypothesis, not executed SQL or authorization
for arbitrary draft reordering, replay or schema-based history substitution.
Retain Task/Project IDs, custom Flow, Started, Session/Flow/Exec history, confirmed
archives and observation ages; reject changed evidence and roll back all ledgers
and data after an injected failure. Then run the required installation harness.

**Measurement:** `a1ac71943` to this working tree adds **0 / removes 0** non-test
production lines; only this working-design entry changes. No runtime consumer or
migration path is replaced. The preceding implementation replaced chapter's
planning reader; this blocked preflight supplies no new behavioral pass. All
acceptance cases 1–15, the two-worker normal-promotion proof, Session discovery,
chapter archival integration and existing policy questions remain unchanged.
No publication, Task disposition, Flow navigation or host promotion occurred.

### Slice review — stacked migration and consumer integration, 2026-09-30

**Blocked; no publication.** Reviewed the complete reconciliation/compression
diff `a9d7ec2fd` → `a7db5b03c`, the Task directive and full acceptance matrix,
and the affected migration, chapter, installation-routing and fixture callers.
The inherited LOO-298 implementation is context, not newly reviewed acceptance.
The required proof stops this review before whole-design acceptance.

- **Executed failure:** `uv run python scripts/test_task_installation.py` exited
  1 on these bytes. Docker returned `29.4.0`; canonicalization ordered 40 drafts,
  but the release build exited 101 at `rust/loopflow/build.rs:95`:
  `build canonical schema at 0.12.25.001_release: no such table: pm_snapshots`.
  No installation, promotion or worker test ran. The harness removed container
  `416e9198832ba454dbb969887a560e2196dede1e4f785178235d7310fd0655c1`;
  a bounded all-container query independently confirmed it absent. The earlier
  Docker outage is not the current blocker.
- **Migration gap:** normalization drops `pm_snapshots` before
  `project_status_chapters` reads it. Simply moving chapter conversion first
  drops `wave_chapters` before `pm_project_evidence` preserves archive receipts.
  Exact applied-draft prefix/checksum validation also prevents a reordered catalog
  from proving adoption of either populated branch frontier. This requires an
  explicit preserving migration integration, not a suffix-only patch or rewritten
  applied history. No migration bytes changed during review.
- **Consumer replacement:** chapter selection now reads the normalized typed
  snapshot; the unused `fetch_pm_snapshot` wrapper and `WorkCatalog::load` are
  gone. Project transitions and completion share one paginated status reader;
  duplicate response DTOs and sorting are removed. The recorded 34 adapter passes
  and subsequent two selection passes remain narrow simulated-provider evidence,
  reused without rerunning unchanged behavior. They do not exercise the failing
  materialized schema.
- **Remaining source gaps:** chapter rotation completes the predecessor but never
  calls `complete_and_archive_project`; that adapter has only test callers.
  Retained-Session discovery still resolves manifests instead of AgentSession
  identity. Normal promotion still stops at status/open identity and cwd checks,
  without exact review completion, a second worker or its runtime digest.
  Multiple physical execution matches still all refuse, including ordinary copied
  installation data. These are outstanding integration work, not new policy
  choices or reasons to select a copy by equality or timestamp.

**Measurement:** `a9d7ec2fd` → `a7db5b03c` adds **63 / removes 114** non-test
physical Rust lines across six files. The compression subset
`6a2f7eae6` → `a7db5b03c` is **+38 / −64**. Counts compare production prefixes
before trailing test modules, excluding test files, scripts, docs, scratch,
generated artifacts and inherited parent changes; no rename credit. This final
range replaces the earlier working-tree reconciliation count. The previous
normal-promotion fixture pass (`83dea4b9c` → `1edec5de0`) was **+0 / −0** production
Rust with no consumer replacement. The current pass does replace chapter's
planning reader, so the two-consecutive-no-replacement condition does not apply.

Next implement the preserving migration integration and prove released history
plus both populated draft frontiers in disposable source/materialized stores.
Then rerun the installation harness and finish normal-promotion succession through
two workers using Exec/AgentSession/FlowSession evidence. Retain the divergent-copy,
obsolete-executable and PTY cases, archive retry/history, all acceptance cases 1–15
and the pending relationship-repair decision. `git diff --check` passes; existing
formatting/Clippy receipts remain dated evidence. No executable edit, installed-Home
access, host promotion, PR publication, Task disposition or Flow navigation occurred
in this review. The saved decision step owns the response to these findings.

### This slice — LOO-298 stack integration, 2026-09-30 (blocked)

Jack Heart's authorized local rebase is complete onto remote
`jack-heart/data-model-one-table-per` at `b21f657fc61517d3eaa3718e5938b27416c21048`.
The local parent branch had newer unpublished work; it was not used or edited.
Incoming notes were preserved at `ace478084` before replay; replay ends at `a9d7ec2fd`. PR #1354 now targets
`jack-heart/data-model-one-table-per`, verified through GitHub readback. Installed
`lf` has no standalone retarget command; the exact authorized base-only mutation
used `gh pr edit 1354 --base jack-heart/data-model-one-table-per`. No branch push,
publication, merge or host promotion occurred; GitHub's head still predates this
local integration.

**Integrated concepts:** retained LOO-298's Exec/AgentSession/FlowSession and shared
ordinary command execution, provider-backed Project chapter selection, and removal
of daemon/webhook ingress and old Swift contract tests. Chapter readers now consume
normalized planning directly and represent absent issue acquisition explicitly.
Removed the unused snapshot-fetch wrapper and duplicate Project-status DTOs.
The removed webhook transport test now exercises stored change receipts directly;
it no longer claims signed ingress. Its assertions remain unexecuted on this tree.
Parent assumptions survive at [data-model-questions.md](data-model-questions.md).

**Required proof failed before promotion:**
`uv run python scripts/test_task_installation.py` exited 1 after materializing the
40 combined drafts in a disposable Docker source copy. The release build panicked:
`build canonical schema at 0.12.25.001_release: no such table: pm_snapshots`.
Container `b1f4c812f2069852b13398c0e1d97bb40b3ef1a83ee00dc0fd183c19c246a83d`
was removed by the harness. Docker 29.4.0 responded; the earlier
`e6e1f0a11850` container was already absent. This replaces the runner blocker with
a reproduced migration-composition failure. No installation or worker test ran.

**Counterexample to a simple rebase:** `normalize_pm_planning` drops
`pm_snapshots`; parent `project_status_chapters` still reads/updates that table.
Moving the latter first is insufficient: it drops `wave_chapters`, which
`pm_project_evidence` reads to retain confirmed archives. The development migrator
also requires an exact applied-draft prefix, so reordering cannot establish
preservation of the two existing draft frontiers. Neither branch's migration SQL
or applied checksums was rewritten. A suffix-only draft cannot repair a failure
that occurs before reaching it. Stop dependent runtime/ownership work until an
explicit migration integration handles released history and both populated draft
frontiers. This is technical implementation work, not permission to erase history
or to choose copy succession from equality or timestamps.

**Observed checks:** `cargo fmt --check`,
`cargo clippy --all-targets -- -D warnings`, `git diff --check` and
`uv run python scripts/check_migrations.py` pass. The migration checker confirms
55 shipped files unchanged and draft structure; it does not execute the combined
SQL and therefore does not contradict the harness failure.
`cargo test -p loopflow --lib complete_and_archive_project` passes both adapter
cases. This narrow proof validates retained refusal/confirmation behavior only;
the parent's chapter operation does not yet consume that archival adapter.
No affected suite, full gate, live provider or installed acceptance ran.

Compression shares Project-status pagination while retaining team preference for
transitions and exact scope for completion; selection uses a minimum instead of
sorting. Production Rust adds 38 / removes 64 lines, excluding tests. All 34 Linear
adapter tests passed, then both selection tests passed with added fallback coverage;
formatting, all-target Clippy and diff checks pass. These are simulated-provider
proofs; the combined migration failure and installation cases above remain open.

**Measurement:** compile reconciliation after replay (`a9d7ec2fd` → working tree)
adds **29 / removes 54** non-test physical Rust lines. Counts exclude test modules,
standalone test files, scripts, docs, scratch, generated artifacts and inherited
parent work. Rebase replay itself is not counted as new authorship or deletion.
The switched production consumer is chapter planning's typed normalized snapshot
read; this pass does not establish a replaced worker/discovery path.

**Remaining integration:** installation fixtures compile against FlowSession but
retain unexecuted historical manifest/Run-event assumptions and need actual
AgentSession/Exec evidence, exact review completion and the second worker. The
normal-promotion fixture no longer requests the removed daemon artifact. Runtime
Session discovery still needs to replace its manifest lookup with the integrated
Session owner, preserving Ask, ordinary Flow and artifact aliases. Restore chapter
archive retry/preservation proof on provider-backed rotation; do not reintroduce
chapter receipts. Port execution/action/Swift fixture coverage to the surviving
consumers. The harness's former six-case receipt predates this model and is not
reusable. All acceptance cases 1–15, local/contextual planning, recursive locks,
delayed startup and the command story remain required. Relationship repair remains
with its existing pending Ask; no new product policy, Flow verdict or Task
disposition follows from this blocked integration.

### Unblock direction — 2026-09-30

Jack Heart's supervising session relayed his instruction to stack LOO-334 on
LOO-298: use `lf rebase` onto `jack-heart/data-model-one-table-per` (PR #1296),
resolve conflicts toward LOO-298's model, retarget PR #1354 to that branch, then
continue ownership work against its execution owners. This supersedes waiting
for integration through main and authorizes that specific PR retargeting.
The [Ask feedback](installation-continuity-unblock.md) records the direction and
fresh recovery evidence: Docker returned server 29.4.0; the exact stranded proof
container was removed and confirmed absent without restarting the service.
The caller still owns stack integration and the two-worker promotion proof.
All acceptance cases, the full-code publication boundary and the separate pending
relationship-repair decision remain. No rebase, PR mutation or behavioral proof
was performed in this Ask.

### Slice review — normal promotion prerequisites, 2026-09-30

**Blocked; no publication.** Reviewed the complete current-slice diff
`53fbf5435` → `b1719ea6c`, the supplied directive/design and prior receipts,
and the installation/discovery callers. The required proof stopped this review
before a fresh whole-branch review or behavioral acceptance could finish.

- **Executed gap:** `uv run python scripts/test_task_installation.py` exited 1
  at Docker preflight: `Docker did not respond within 10 seconds; no proof
  container was created.` No test or promotion ran in this attempt. The corrected
  promotion fixture and divergent-invocation extension remain unverified. The
  previous six-case pass predates both changes and cannot validate them.
- **Source gap:** the new fixture captures operation → review → operation but
  stops after post-promotion status/open assertions. It neither completes the
  review nor executes the second worker, poisons inherited runtime selection,
  or checks that worker's digest. Even a future pass of this fixture alone will
  not satisfy cases 6 or 15. The earlier obsolete-command and PTY cases remain.
- **Ownership gap:** `existing_execution` still rejects every multiple physical
  match. `promote_local_candidate` can reuse development data or copy production
  independently of `prior.store`; `disposable_store_owned` records cleanup, not
  execution succession. These source observations support the existing finding,
  not a newly observed promotion failure or permission for equality/timestamp
  selection. LOO-298 revision `d61295196` is still not an ancestor of HEAD
  (exit 1); last-fetched `origin/main` remains `a3820bf7e`.
- **Measurement:** current slice adds **0 / removes 0** non-test production Rust
  lines. Its only Rust diff is the integration-test file (+258/−29); scripts,
  docs, scratch and generated files are excluded from the production count.
  No production consumer or predecessor path is replaced in this pass. The
  preceding implementation `01158f839` → `53fbf5435` replaced installed
  Task/Session routing and captured Session forwarding. Compression at
  `b1719ea6c` belongs to the current pass, not a second implementation pass;
  the two-consecutive-no-replacement blocker does not apply yet.

`git diff --check` passed. Prior formatting/Clippy and Session results are retained
as dated evidence, not rerun or upgraded. No code changed during this review.
The earlier stranded container's cleanup remains unconfirmed; this attempt did
not inspect/remove it or restart Docker.

Next resolve Docker availability, remove the exact previously recorded proof
container, and run the required harness. Then finish the installation-copy
ownership cut against integrated execution owners and extend the same proof
through the second actual worker. Return this blocker to the saved decision step;
do not repeat an unchanged implementation pass. All cases 1–15, recursive locks,
delayed startup, local/contextual planning and command-story obligations remain.
The existing relationship Ask and other recorded policy choices are unchanged.
Jack Heart's full-code publication boundary remains unmet; this review selects
no Flow navigation, Task disposition or installation promotion.


### This slice — normal promotion prerequisites, 2026-09-30 (blocked)

Compression: shared the fixtures' unbound CLI setup and the harness's exact-test
command; removed transient build history from TESTING.md. Production routing and
assertions are unchanged. Formatting, all-target Clippy, Python/shell syntax and
whitespace checks pass. The harness retry stops at Docker's ten-second preflight;
no new container or behavioral result. Earlier container cleanup remains unconfirmed.

Preserved the supplied terminal-forwarding repair, PTY fixture and documentation
in `53fbf5435` before editing. The current pass adds a real-promotion regression
and corrects the existing divergent-copy fixture; it changes no production routing.
The full Done When and acceptance cases 1–15 remain unmet. Publication is not ready.

**New source observation:** `promote_local_candidate` normally reuses a selected
development database. When creating a development installation, it backs up
`production_database_path()`, which need not equal `SwitchReceipt.prior.store`.
Published promotion selects production again without transferring development
execution. The receipt's `disposable_store_owned` records cleanup ownership; it
contains neither an execution transfer nor the copied execution baseline. Thus
`prior → target` alone cannot authorize choosing one of two Task copies. A receipt
plus equal Task IDs also cannot detect later independent writes. Stop the dependent
succession resolver change; do not add the equality shortcut rejected at review.
This is source evidence, not a reproduced normal-promotion failure or a claim that
all possible succession evidence has been exhausted.

**Authored proof:** `normal_promotion_preserves_pending_task_review` starts from an
empty disposable account using actual published promotion, authors cached planning
and a captured operation → review → operation Flow, then starts the first real
worker. At the pending review it invokes ordinary development promotion through
`install promote --from-build`, checks both retained Flow positions, and requires
public status and Session open to retain the original Task/review. The harness
materializes migrations only in its container and builds separate release-provenance
and development pairs. No fixture-written switch receipt substitutes for promotion.
The existing empty-B test now creates a different captured invocation in its backup
before asserting ambiguity; its earlier identical-copy interpretation was wrong.
The obsolete executable and PTY regressions are preserved.

**Observed verification:**

- `uv run python scripts/test_task_installation.py` first reached the new test but
  failed before promotion: development bytes correctly refused production data.
  Replaced that invalid fixture with actual release-provenance bootstrap. A second
  setup attempt used the member manifest for the workspace version and failed with
  an empty version; corrected to root `Cargo.toml`.
- The corrected attempt materialized all three planning drafts inside the container
  and began the release-provenance build. Docker then stopped answering, including
  a ten-second `docker info` and inspection of this exact container. No corrected
  promotion, Task continuation or divergent-copy result was reached. The original
  six-case receipt remains prior evidence, not a current-tree pass.
- Interrupted the owned Python driver and its blocked local Docker clients. A
  separate `docker rm --force` timed out after 30 seconds. Cleanup of container
  `e6e1f0a11850241a3e010fe6e7acbd7d79054b1514c080e8664fd421ef3311a6` remains
  unconfirmed. Do not restart the shared Docker service or claim the container is
  absent. The harness now bounds cleanup to 30 seconds and limits Cargo to two jobs;
  these changes are not evidence of the outage's cause or a successful retry.
- Host formatting, all-target Clippy, Python syntax and whitespace checks passed.
  No affected-suite or repository gate ran. No branch binary touched the host
  installed Home; both attempted installation targets were container-owned.

Compared with `53fbf5435`, this pass adds **0** and removes **0** non-test production
Rust lines. All Rust edits are integration-test code; the harness, docs and scratch
are excluded. No production consumer is replaced in this pass. The immediately
preceding pass replaced Task/Session routing, so this is not two consecutive
no-replacement implementation passes. Do not count preserved prior repairs as new
implementation.

`git merge-base --is-ancestor d61295196 HEAD` returned 1. The fetched main remains
`a3820bf7e`; the received LOO-298 revision is present only on its feature refs.
No fetch, integration, owner-branch edit or shared execution migration occurred.
After Docker recovers, remove the exact stranded container and rerun the required
harness. Finish the normal-copy ownership cut and two actual worker boundaries
before locks/startup and broader discovery. Ask/ordinary Flow discovery must use
the integrated stable owners; do not build another schema here. All local lifecycle,
contextual Wave, managed-validity, DTO/action, migration and command-story obligations
remain. Relationship repair stays with the existing Ask; outage/connection/baseline
choices are unchanged. No publication, host promotion, Task completion or Flow
navigation follows from this blocked proof.


### Slice review — installation continuity, 2026-09-30

Reviewed the active diff from `a3820bf7e` through `b79d3cce4`, concentrating on
the installed-continuation slice since `01158f839`. **The narrow installed Task
and review proof passes after a terminal repair; the full Done When is unmet.**

- **Pass:** `uv run python scripts/test_task_installation.py` passed all six
  disposable cases before the extension and again after the repair. Public Task
  status/run and review open/complete retain A's identity and invocation while a
  real B worker completes the saved operation. The test uses fixture-authored
  installation receipts, simulated tmux and cached planning, not normal promotion
  or configured providers. Earlier planning/chapter proofs are reused, not rerun.
- **Fixed:** Session forwarding reused `execute`, which supplied null stdin,
  captured stdout and detached the process group. Extending the existing case
  with a real pseudoterminal and simulated OpenCode reproduced provider exit 17
  because stdin/stdout were not terminals. Session commands now replace the
  process on Unix through `forward_session`; the shared command builder retains
  environment sanitization and execution routing. The final case reads the exact
  supplied terminal input through the public `session open` path. The Session
  consumer no longer uses captured Task output. This proves terminal I/O, not
  real provider conversation or recursive runtime locks.
- **Static proof:** `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`
  and `git diff --check` pass. Initial macOS Clippy rejected the fixture's const
  null pointers to `openpty`; mutable null pointers corrected that platform
  signature mismatch. The Linux harness retains the existing unused `complete`
  warning. Resource preflight passed; no affected-suite or repository gate ran.
- **Measured replacement:** `01158f839` → `b79d3cce4` is +262/−66 non-test Rust
  lines across ten files; this corrects the earlier +265/−35/eight-file report.
  This review adds +37/−12 across two production files; combined slice +292/−71.
  Counts exclude test modules and inline test-root setup, integration tests,
  scripts, docs, scratch and generated artifacts. The slice replaces local-only
  installed Task/Session routing, the current-executable runtime preference and
  WorkCatalog's Run-ledger prerequisite. The prior chapter pass
  `44b2ec74e` → `01158f839` replaced archive-only closure. Neither pass is a
  no-replacement pass; the convergence blocker does not apply.

**Remaining findings and next proof:** the harness's final `Backup` creates an
identical execution copy and expects refusal; it supplies no evidence of actual
divergence. `_copy_store_for_candidate` also uses SQLite backup during installation,
so rejecting every physical copy cannot establish Jack Heart's ordinary-switch
requirement. Next exercise normal disposable promotion with a pending review and
two real worker boundaries. Use existing installation succession evidence to
distinguish continuity from genuine divergent execution without timestamps,
store merging or a user-facing database choice. Retain a separate divergent-copy
case. This requires the larger continuation cut, not an equality-based shortcut.

Source review also finds `existing_session` resolves only Task boundary IDs and
Run manifests: Ask IDs and `flow:<invocation>:<boundary>` cannot locate retained
Sessions through it. Complete discovery against LOO-298's integrated owners;
its received contract is still not integrated here. Source-private/remote/legacy
discovery, recursive locks, delayed startup and acceptance cases 1–15 all remain.
Local lifecycle, managed validity/ordinary Flows, contextual Waves, creation/link
hierarchy, execution/action DTOs and the command story are not waived. Relationship
repair stays with the existing pending Ask; outage/transition/baseline choices
remain explicit. Return these findings to the saved decision step. Jack Heart's
code-completion publication boundary is unmet: no publication, landing, Task
completion, promotion or Flow navigation follows from this review.

### This slice — installed Task and review continuity

Compression: shared Task identity matching and installed CLI verification; removed
the unchecked review-launch resolver and its repeated installation lookup.
The routing proof passed without a Run-event table, including changed/missing CLI
refusal; all 19 Session unit tests and six disposable installation cases passed.
Formatting, all-target Clippy and whitespace checks passed. The existing Linux
unused-variable warning and full-design acceptance gaps remain unchanged.

Preserved the incoming nullable-issue decoder/test/review edits in checkpoint
`01158f839`, then implemented the next installation-switch cut. The full
single-PR Done When remains unfulfilled; all acceptance cases 1–15 remain required.

**Switched consumers:** installed `task status <identifier-or-UUID>`, `task run`,
and explicit Session open/complete/rename now search selected and retained
installation stores through the existing WorkCatalog/read-only identity reader
and Session evidence. One retained execution routes to the selected CLI without
copying its Task, invocation or review. The WorkCatalog no longer requires the
unrelated Run-event schema merely to read Work identities. Physical aliases use
existing filesystem identity comparison; multiple physical matches report their
locations rather than selecting by timestamp.

The worker-boundary resolver selects and verifies the current installation's CLI,
independently of the addressed execution database. Installed store opens accept
explicit execution placement after read-only schema validation and cannot migrate
that other database. Task children carry ordinary data routing as well as control
routing. Installation records themselves remain unchanged. This replaces the
current-executable runtime preference and installed-binary/default-database-only
path for these consumers. New-work/default routing and branch-only Task refusal
still exist; they have not been presented as the completed local lifecycle.

**Observed regression and proof:** extended `scripts/test_task_installation.py`
with `installation_switch_preserves_task_review_without_store_overrides` before
changing production code. It first failed: after selecting B, status returned
`execution: null` and unavailable planning while A still held the exact review.
The final harness command passed all six cases. The new case uses two distinct
real CLI artifacts, fixture-authored installation selections, cached planning and
one prepared review. Public status by identifier/UUID, run and Session open find
the original Task/checkout/review with no data overrides. An obsolete executable
rejects `session`; fresh discovery returns a usable continuation through B despite
poisoned PATH, LF_BIN and LF_CONTROL_BIN. Completing the exact ready review then
runs a real B worker with the saved invocation in A. Its existing Run-event
commands identify B; the Flow finishes once and B has no duplicate Task. Readiness
setup addresses the original review directly as fixture preparation; the tested
post-switch status/run/open/complete calls supply no store settings.

The same case adds an unknown draft to A and verifies the incompatible attempt
preserves its DB/WAL bytes and exact review, then removes only that fixture draft.
A second physical execution copy causes explicit ambiguous-continuation refusal.
Tmux transport is simulated and the child performs `rebase --plan`; no configured
provider or real installation promotion is exercised. There is one actual worker
boundary, not the two-boundary case 6. The obsolete saved executable remains
obsolete: this proves a fresh public continuation, not repair of that binary.

**Self-review:** moved execution addressing out of the initial in-memory
InstallSelection rewrite so receipts keep their real artifact/store identity.
Kept source-private status on its local reader: forwarding it to a writable foreign
status operation would violate the source-isolation acceptance contract. Foreign
read-only detail inspection remains work. Installed status now checks discovery
before its local Task read, including conflicts. Removed the normal forwarding
message that exposed database routing as the user's next action. CLI and testing
docs describe these boundaries.

**Proof commands:**

- `uv run python scripts/test_task_installation.py`: six passes after the initial
  expected missing-execution failure. A later fixture extension first failed to
  compile because it supplied Op instead of ConcreteOp; corrected before the final
  passing run. The existing Linux-only unused `complete` warning remains.
- `cargo test -p loopflow --lib ops::task_destination::tests::managed_operations_move_before_branch_effects -- --exact`:
  one pass, including the final rerun after the store-opener refactor.
- `cargo test -p loopflow --lib lf::commands::work_catalog::tests`: one pass.
- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and
  `git diff --check`: passed. No affected-suite/repository gate ran.

Compared with `01158f839`, the implementation through `b79d3cce4` adds **262**
and removes **66** non-test physical Rust lines across ten production files.
The review recount excludes test modules and inline test-root setup as well as
integration tests, scripts, documentation, scratch and generated artifacts.
The current-executable preference and identity
reader's Run-ledger prerequisite are removed; existing general Task preparation
is reused rather than replaced with another execution owner.

**Remaining and integration limits:** discovery currently scans installation
receipts, not known remote routes, legacy/source layouts or the full requested
scope. It reports unreadable candidate stores as errors. Source-private status
still reads its private copy. Identical execution rows copied by normal promotion
are not distinguished from divergent copies; that continuity remains unproven and
currently reports a conflict. The fixture switches to an empty B execution store,
so it cannot settle that case or establish full case 15. A stale already-saved
open_argv is not rewritten in place. Status does not yet expose runtime policy,
and explicit recursive locks, provider-shell propagation, delayed startup and
normal promotion remain unfinished. No pin schema or replacement execution tables
were added. `git merge-base --is-ancestor d61295196 HEAD` returned 1; LOO-298's
received contract is still not integrated. This cut uses the existing Session
reader and needs that integration before dependent owner/migration work.

Next extend the continuity proof to actual promotion/copies and two worker
boundaries, then implement explicit locks and delayed-child admission against
LOO-298's integrated owners. Retain local lifecycle, managed-validity/ordinary-Flow
separation, contextual Wave imports, create/link/hierarchy, DTO/actions, design
placement/handoff and the full command story. Relationship repair still waits on
the existing Ask; outage, connection and baseline policies remain open. No duplicate
Ask, provider mutation, host branch-binary execution, publication, promotion,
Task completion or Flow navigation occurred in this implementation pass.


### Slice review — 2026-09-30

Reviewed the branch diff from `a3820bf7e` through `dc010b78c`, with the current
chapter-completion slice measured from `44b2ec74e`. **Local slice passes; the
full single-PR Done When remains unfulfilled.**

- **Pass:** `apply_rotation` transfers/disposes work, completes predecessors, then
  archives and records their retirement through the existing chapter operation.
  Refused completion and lost completion/archive responses preserve retry and
  dated history. The sole consumer now calls `complete_and_archive_project`;
  `archive_project` is deleted. Compression also removes the redundant
  `store_pm_snapshot_with_store` wrapper and reuses one refresh/store path.
- **Fixed:** a GraphQL response containing `data: {}` decoded as `issue: null`,
  incorrectly reporting absence and invalidating cached planning. Extending
  `omitted_detail_fields_do_not_clear_known_planning` reproduced
  `task "FIX-1" is absent from repository planning`. The existing decoder now
  requires the nullable `issue` field. A malformed response fails acquisition
  while preserving the prior record and age; explicit null retains its existing
  absence semantics. This adds no storage or admission policy.
- **Executed proof:** `cargo test -p loopflow --lib ops::pm::planning_lookup_tests`
  passed all 11 cases after the repair; the focused omission test failed before
  it. `cargo test -p loopflow --lib chapter_rotation_previews_retries_and_preserves_dated_history -- --nocapture`
  passed one stateful case, and
  `cargo test -p loopflow --lib complete_and_archive_project` passed two cases.
  `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and
  `git diff --check` passed. These use disposable stores and simulated Linear;
  no configured Project was completed. Earlier public-CLI, migration and DTO
  receipts remain supporting evidence, not reruns or full acceptance.
- **Measured replacement:** `44b2ec74e` → `01d4894a2` is +153/−6 production Rust
  lines; through `dc010b78c`, +158/−23 across chapter, Linear and PM operations.
  This review adds one production attribute (+1/−0). Counts include non-test
  physical lines and exclude test modules/files, docs, scratch and generated
  artifacts. The previous implementation pass `34a8eb7b2` changed only research
  notes and replaced no consumer; the current pass replaces chapter archival.
  Thus the two-consecutive-no-replacement blocker does not apply.

**Remaining findings and next cut:** source still couples runtime/database in
`task_destination::destination` and `resolve_current_home_lf_binary`, and releases
the startup claim after the ten-second observation error. `RegistryQuery.taskStatus`
has no app caller yet; planning DTO tests do not establish execution/action parity.
Switch ordinary Task status/run and Session continuation onto bounded execution
discovery and independent runtime selection, using LOO-298's received owners.
First add acceptance case 15 to the disposable installation harness, then prove
the unchanged review/invocation across selection and the selected next worker.
Cases 6–8 retain recursive-lock and delayed-child obligations. Local lifecycle,
contextual Wave imports, command-story handoff and all other acceptance cases
remain required. Relationship repair still awaits the recorded decision; outage
and transition choices remain open and were not inferred here.

`uv run python scripts/test_task_installation.py` was not rerun: its present five
cases omit the required installation-switch/runtime/lock scenarios. No environment
failure is claimed; the missing proof is implementation work. Jack Heart's full
code-completion publication boundary is unmet, so this review does not publish,
land, complete the Task or choose Flow navigation. Return this evidence to the
saved decision step; do not repeat the finished planning/chapter slice as the
next implementation pass.

### Earlier slice — predecessor completion, 2026-09-30

Continued from `44b2ec74e`, preserving the supplied unfinished chapter/Linear
edits before extending them. `apply_rotation` now completes each predecessor
Project after transfer/backlog disposition, then archives it through the existing
retryable chapter operation. The former archive-only adapter entry point is
removed. Completion selects the first completed status by provider position and
ID within the current status's team/workspace scope, acquiring every status page.
Confirmed completion and archive acknowledgement remain separate evidence.
No schema or execution ownership changes are introduced.

Self-review against the pinned [Linear SDK schema](https://github.com/linear/linear/blob/b37823be308a42f837277671f3ded66d33d92e6c/packages/sdk/src/schema.graphql)
found that `ProjectPayload.project` is nullable. The adapter
now reports an unconfirmed completion for null or non-completed mutation results,
even with `success: true`; it cannot continue to archive. The existing stateful
chapter proof covers refused completion, lost completion response, lost archive
response, normal completion on the following chapter, and retries preserving
Task/PR identity and dated KR/metric history. Statuses on another team or workspace
cannot supply a completion status for the fixture's team-scoped Project.
Previously completed chapter receipts are not rewritten or treated as proof that
historical provider Projects received the newly added completion mutation.

The full single-PR design remains unfinished. This slice does not establish Task
discovery, official-runtime switching, recursive locks, local lifecycle or the
command story. `task_destination.rs`, `engine/process.rs`, and installed-store
resolution still couple the runtime to its installation's database. Normal Task
and Session operations must remove that need for manual routing; a diagnostic
reach command alone no longer meets Jack's clarified acceptance requirement.

Proof for this slice:

- `cargo test -p loopflow --lib chapter_rotation_previews_retries_and_preserves_dated_history -- --nocapture`
  passed (one stateful chapter test, including the new failure/retry paths).
- `cargo test -p loopflow --lib complete_and_archive_project` passed (two adapter
  tests, including success-with-null/non-completed results and archive refusal).
- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and
  `git diff --check` passed. No affected-suite or repository gate ran.
- Compared with `44b2ec74e`, production Rust is +153 / -6 lines across
  `ops/chapter.rs` and `pm/linear.rs`, excluding test modules, docs, scratch and
  generated files. This includes the preserved incoming edits; it is not all new
  authorship in this pass. The archive-only public adapter and its caller were
  replaced, with no remaining `archive_project` method/call.
- Full-design command `uv run python scripts/test_task_installation.py` was not
  run in this slice: its existing cases still do not implement the required
  installation-switch/Session/recursive-lock acceptance matrix. The new chapter
  proof uses a simulated provider and disposable database; no configured Linear
  mutation or installed-Home source execution occurred. The full Done When is
  unfulfilled. Relationship-repair and outage/transition decisions remain open.

Compression reused snapshot refresh and one store handle, kept only the selected
completion status across pages, and shared planning-test snapshot setup. Chapter,
completion-adapter, planning-lookup, OAuth-refresh and PR-identity checks passed
(16 tests); the PR fixture first failed on its same-revision URL replacement and
now supplies a newer observation. Formatting, all-target Clippy and whitespace
checks passed. The full-design acceptance gaps above remain unchanged.

### Execution contract and recursive locks — 2026-09-30

Jack supplied successful LOO-298 coordination from a bounded read-only contribution
at `4f4edff9b` through `d61295196`; the intervening change was documentation only.
The earlier unregistered response below is historical, not a current coordination
blocker. Task owns stable identity/Project/Wave/checkout/PR evidence. AgentSession
and session_events own conversation/captured input/native execution/usage;
FlowSession and flow_events own captured graph/cursor/claims/review; Exec owns
process evidence. Runtime Run storage is removed there. The approved removal of
runtime child FlowSessions and preserving migration are still outstanding. Put
neither discovery nor lock policy on child FlowSession identity.

Use WorkCatalog/work_identities for discovery, TaskExecutionSnapshot for claim and
process evidence, task_run for continuation, and the shared skill-command executor.
Local planning must preserve Task and PR/Session/Flow/event/import identities.
Keep forward draft checksums and prove released-history and draft-Session frontiers
separately. Claim acquisition alone is not Started, and driver death does not prove
provider death. The contribution establishes a source contract, not tests, provider
acceptance or integration. This branch still has the earlier execution model;
LOO-298 remains unfinished and unaccepted, and its checkout must remain untouched.

Jack's recursive-lock steer requires PATH propagation through all descendants,
including commands typed by an agent. Source tracing here finds all three provider
launchers calling `configure_vendor_std_env`; development launches currently rebuild
PATH from the parent process, potentially replacing `AgentConfig.env`'s PATH.
Codex additionally disables login shells and shell snapshots. Those findings do not
prove recursive lock behavior. The selected contract and new acceptance cases
below require real child/shell evidence for Codex, Claude and OpenCode. Jack reports
that LOO-298 is moving its Flow children to PATH lookup to honor this contract.

### Earlier planning checkpoint

Reconciled against `01f7814ef`, including the planning-evidence implementation
at `3508257b0` and its subsequent compression. The connected
planning slice is implemented; the full approved single-PR outcome remains open.
The last-fetched `origin/main` is still `a3820bf7e`. No new integrated LOO-298
execution contract or owner reply was found; no fetch or coordination retry ran
in this reconciliation. Shared execution migrations remain blocked.

### Implemented behavior

- Exact connected lookup acquires an issue before resolving ownership, stores
  Project-less issues and resolves identifier/UUID aliases without execution
  allocation. PM operations and Task lookup share the same reader and ownership
  resolver. Confirmed writes still force provider acquisition.
- Normalized repository/provider/UUID entities replace `pm_snapshots` payloads.
  Wave lists join Project membership to the shared entities in one read transaction.
  Forward migration preserves prior observations and confirmed-deletion fences.
  Detail reads preserve existing rank within a Project; list omission never erases
  Task facts. Required nullable fields prevent partial responses clearing facts.
- Linear list and detail use one issue decoder. Parsed `updatedAt` revisions order
  issue facts at nanosecond precision. Older/unknown revisions cannot overwrite
  newer known facts; contradictory equal-revision facts fail without replacement.
- Verified issue webhooks persist UUID-scoped revision/removal receipts even
  without a cached issue or execution. Complete detail at or beyond the revision
  repairs invalidation; partial events never replace complete entities. Removal
  receipts fence later ingestion, including events received before caching.
  Existing steering/inbox owners remain unchanged. No execution schema changed.
- Task status returns planning, optional execution, `planning_state`,
  `planning_stale` and `planning_error`. Inspection preserves the prior observation
  age through soft/hard/forced refresh failure and exposes invalid/removed facts.
  Status observes PRs without completing Tasks.
  Rust/Swift planning fixtures and `RegistryQuery.taskStatus` cover the envelope;
  Swift has a typed execution projection, not complete action/runtime parity.

### Planning-evidence follow-through — 2026-09-29

Task status now uses the shared observation reader and returns `planning_state`
(`available`, `invalid`, `removed`, `absent`, `unavailable`) with optional retained
facts. Hard-stale and forced inspection failures retain last-good facts and their
original acquisition date. Uncached resolution failures produce an unavailable
status envelope without execution. A null detail response reports scoped absence;
only confirmed removal receipts establish removed state. Managed readers filter
invalid/removed observations and retain the existing hard/forced refresh refusal.
This does not select cached-Task outage admission.

Realign reproduced a concurrent-observation counterexample: a confirmed removal
arriving during an in-flight detail lookup was overwritten in the returned state
by the lookup's null response (`Absent` instead of `Removed`). Inspection now
preserves the removal receipt's state and dated facts. The regression uses the
existing test-server barrier, not timing sleeps, and also checks that managed
reads refuse the Task and no execution is allocated. This repair adds no storage,
restoration or outage policy.

Project list/detail queries now acquire `updatedAt`. Shared writes order Project
facts separately from issue revisions, so a newer Project observation can win
when its enclosing issue request began earlier. Missing Project content or
relationship fields fail acquisition instead of clearing known data. Unknown or
older revisions cannot overwrite known facts. Equal-revision contradictions fail.

Membership no longer uses list acquisition time as replacement authority. A list
omission without removal evidence is unresolved. Initiative/Team relationship
changes cannot be ordered by Project `updatedAt` alone: the writer retains known
facts and persists unresolved ownership, excluding it from managed readers. List
and detail replays cannot clear that state. Acquiring relationship-specific
revision/removal evidence and its repair remains outstanding; this implementation
intentionally does not guess a winner or silently reconcile ownership.

Review caught two concrete integration hazards and fixed them: Project writes
were conditional on accepting the enclosing issue, and treating every Project
omission as unresolved would also break confirmed chapter archival. Project
writes now have independent revisions. The existing chapter archive operation
records its successful provider acknowledgement in the planning store; current
views exclude the predecessor even after delayed lists. A forward planning-only
migration carries completed chapter archival receipts into that evidence.
Historical facts remain. Provider completion (distinct from archive), external
archive acquisition and restoration semantics remain separate unfinished work.

The self-review also preserved execution inspection when the owning checkout is
missing; planning-context failure cannot hide already-recorded execution.
No execution schema, runtime selection, execution ownership, account or live
provider state was changed by this follow-through. No LOO-298 coordination retry
was needed for this planning-only schema change. Its execution-contract blocker
remains unchanged. Infrastructure memory now reflects these implemented inspection
and Project-revision contracts and the archival/canonicalization lessons; its
earlier hidden-invalid and acquisition-ordered Project guidance is superseded.

Focused local proof receipts:

- `cargo test -p loopflow --lib ops::pm::planning_lookup_tests`: all eleven cases
  passed, including forced failure, retained invalid/removed/absent facts,
  partial Project fields, independent Project revisions and removal during a null
  lookup. Realign reran this group after the new race case first failed with
  `Absent` instead of `Removed`. The ordering case had previously been rerun
  after distinguishing unknown revisions from known older revisions.
- `cargo test -p loopflow --test planning_lookup_tests --test dto_fixtures`:
  public CLI matrix passed; all eleven DTO tests passed. CLI cases include
  identifier/UUID, more-than-seven-day-old planning, invalidation, cached and
  uncached removal, and unavailable planning without execution. No execution,
  execution Project or extra worktree was allocated. Forced inspection is a
  shared-reader proof; Task status has no new refresh flag.
- Swift `ContractTests.taskStatusPlanningFixture`: passed for the six evidence
  states and Project revision. Existing Ghostty missing-symbol and macOS
  CVDisplayLink deprecation warnings remain outside this change.
- `chapter_rotation_previews_retries_and_preserves_dated_history`: passed with
  acknowledged archival excluded from the active view. Its old delayed-list
  assertion now expects unresolved omission while preserving current membership.
- `migration_preserves_planning_identity_and_removes_snapshot_storage`: passed,
  including populated chapter archival receipts and delayed predecessor ingestion.
- `missing_worktree_status_is_actionable_and_read_only` and
  `repeated_status_of_merged_task_never_completes_work`: passed.
- `uv run python scripts/check_migrations.py`, `cargo fmt --check`,
  `cargo clippy --all-targets -- -D warnings`, and `git diff --check` passed.
  Scratch links resolve and the full acceptance matrix is unchanged.

The other behavioral receipts above are reused in this reconciliation; DTOs and
public command shapes are unchanged by the race repair. Realign also passed
`cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, whitespace and
local-link checks, and verified the full acceptance matrix is unchanged.
These are simulated-provider and disposable local CLI proofs. The complete
acceptance matrix remains required and unproven. No affected-suite/repository
gate, live provider demonstration, installation promotion or Flow navigation is
claimed here.

Compression follow-through (2026-09-29): managed planning reads now return the
record directly; inspection owns stale/error reporting. The unused synchronous
managed reader and its duplicate result wrapper are removed. Status resolves
execution once, preserving explicit deleted-Task history lookup. Wave list reads
take Project display names from their existing join rather than searching the
Project vector for each issue. Membership conflict evidence deliberately stays
outside the ingestion transaction so rejection cannot erase that uncertainty.

Review reproduced obsolete fixture writes that erased Projects or replaced known
ownership. The fixtures now use archive/revision evidence or acquire a distinct
ambiguous Project. Removing duplicate initialization setup then exposed a real
scope bug: canonical Wave-path repair left normalized facts under the old alias.
The existing repair now moves all Waves and planning entities under that alias
atomically. It does not merge conflicting observations or move planning between
different repositories. The initialization proof passes using the shared fixture.

Focused verification for this reduction: ten planning-reader/writer cases,
two Wave chapter cases, populated migration and public deleted-history lookup
(14 library tests); public planning-only CLI status; all eleven Rust DTO tests;
repository ownership CLI refusals; repeated merged-PR status without completion;
missing-worktree inspection; and initializing-worktree status/wait/roadmap.
Project rename assertions cover both joined list and detail display names.
The repository-alias proof additionally retains populated planning through
canonicalization. Swift shapes are unchanged, so the recorded Swift proof applies.
No execution migration, provider mutation, outage-policy selection or full gate
follows from this reduction. Cross-repository Wave relocation with populated
normalized planning still needs its own integration proof; the existing relocation
case uses empty planning for the relocated Wave.
Formatting, `cargo clippy --all-targets -- -D warnings` and whitespace checks pass.

The next compression removed `Store::pm_task`, a second filtered reader used
only by tests. Revision/webhook and unresolved-ownership proofs now exercise
`read_task_planning_async`, the actual managed reader. Store migration and alias
proofs inspect `pm_task_observation` directly; removal retains dated facts while
excluding them from current lists. The public CLI case keeps its inspection and
allocation assertions and drops the duplicate filtered-store check. No acquisition,
admission, storage, DTO or relationship-repair policy changes accompany this removal.

### Relationship acquisition contract gap — 2026-09-29

The next implementation pass inspected Linear's public SDK schema at
[`b37823be308a42f837277671f3ded66d33d92e6c`](https://github.com/linear/linear/blob/b37823be308a42f837277671f3ded66d33d92e6c/packages/sdk/src/schema.graphql)
and its [webhook contract](https://linear.app/developers/webhooks). The schema was
downloaded at that exact revision and compared with the inspected master copy;
both have SHA-256 `cc4263f66d6e79f188b1e6b08af5f0fe8dd32dd3c0cdae3c070606e8d1e5e0eb`.
This is public contract research, not authenticated provider behavior or a live
mutation experiment.

Observations:

- `InitiativeToProject` supplies join identity, endpoints, `createdAt`, `updatedAt`
  and nullable `archivedAt`. Project-scoped and workspace-wide join connections
  support pagination and `includeArchived`. The delete mutation returns
  `DeletePayload`; the schema does not establish that a deleted association remains
  queryable as an archived join or supplies a recoverable deletion revision.
- `Project.teams` returns Team entities, without a Project–Team join revision.
  `Project.history` exposes timestamped records with `entries: JSONObject!`, but
  the inspected contract does not define those entries' Team/Initiative deltas,
  retention, completeness or an ordering checkpoint for a current relationship set.
- Project `archivedAt` is affirmative archive evidence. The existing adapter
  queries it only on direct Project ownership, then discards it when converting
  to `PmProject`; issue detail and Project lists do not acquire it. This is a
  concrete acquisition gap independent of ownership repair. A null value alone
  does not establish an ordered restoration under the accepted contract.
- Documented webhooks cover Projects and Initiatives, but do not promise join
  removal events, Project–Team membership revisions or a replayable complete event
  log. Delivery IDs/timestamps and mutation `lastSyncId` do not establish a read
  checkpoint protocol. No documented guarantee was found that Project `updatedAt`
  orders either relationship set.

Consequently the public contract inspected here does **not establish the required
repair authority**. This is not a claim that Linear can never supply it. Adding join
timestamps would handle positive join facts but would not safely replace the whole
Initiative set or repair Team ownership. Simulated responses cannot fill that gap.
Current nested connections also stop at 50 without acquiring `pageInfo`; any
replacement-set proposal must first prove complete acquisition and reject partial
or failed reads.

Decision requested through `lf ask`: retain strict relationship revision/removal
proof and keep disputed ownership unresolved pending a provider contract, or
explicitly revise the requirement to permit a newly acquired complete Project
relationship set to repair ownership. The latter would need local fencing against
already-in-flight observations and honest freshness, but still cannot promise
server-side total ordering or snapshot consistency from the documented API.
This is a product consistency decision, not an implementation assumption.

Dependent relationship implementation is stopped. No production code, schema or
tests changed in this pass, and unchanged behavioral tests were not rerun. Existing
proof receipts remain applicable; legitimate ownership repair, delayed-response
fencing and external archive acquisition remain unproven. Chapter archive receipts
and independent Project/issue ordering are unchanged. No shared execution migration,
provider mutation, publication, promotion or Flow navigation occurred. The complete
single-PR acceptance matrix and all other remaining scope below are retained.

### Remaining implementation

1. Acquire ordered Initiative/Team relationship evidence and an explicit repair
   for unresolved Project ownership. The public-contract gap above requires a
   bound decision before choosing repair authority. The current writer retains uncertainty
   instead of replacing membership by acquisition time. External Project archive
   observations also need acquisition; known chapter archive acknowledgements are
   integrated. Wave `synced_at` still does not date newer joined issue facts.
   Public invalid/unavailable/absent inspection and Project fact revision ordering
   are implemented in the follow-through above.
2. Implement genuine local Task/Project identities and lifecycle through the same
   store. The current reader acquires Linear observations; it does not supply the
   approved local-only lifecycle. Resolve connection migration controls before
   publishing any private planning. Coordinate any execution identity changes
   with LOO-298's owner first.
3. Resolve cached-Task outage admission before enforcing validity at every managed
   launch/resume/worker boundary. Prove ordinary Flow and Session independence
   from invalid/terminal managed Tasks. Do not infer execution permission from
   retained inspection or cache invalidation.
4. Integrate against LOO-298's received execution contract, then implement bounded cross-store
   discovery, independent official-runtime selection, deliberate pins and delayed
   startup. Preserve existing identities, claims and captured invocations.
5. Implement contextual Wave imports and the remote-main baseline, explicit
   create/link operations and portable Initiative hierarchy. Predecessor completion
   after transfer is implemented with simulated-provider proof; configured provider
   acceptance remains open. Resolve remaining baseline/transition choices in
   [questions.md](questions.md) before dependent behavior.
6. Finish design auto-placement, launch-plan artifact handoff, full execution/action
   DTO fixtures and the two-store public-CLI command story. The complete acceptance
   matrix below is unchanged; these internal slices are not separate deliveries.

### Proof and counterexamples

Recorded proofs at `2976e1d34` and `b8e0e1d60` remain baseline evidence. The
follow-through above names the reruns; unrelated proofs were not rerun:

- Eight planning-lookup cases cover detail/list/shared-writer convergence,
  equal-revision conflicts, webhook invalidation, uncached removal, incomplete
  detail, and retained refresh age. List/detail acquisition, creation retry,
  explicit post-merge completion and webhook steering passed after compression.
- All 12 active planning-mutation tests passed (one child entry is exercised by
  its parent). Public CLI identifier/UUID planning-only and stale-status cases,
  repeated merged-PR inspection without completion, missing-worktree/PR-cache
  failure, and the corrected conflicting-Project-ownership matrix passed.
- Rust DTO fixtures (11 tests), Swift `ContractTests.taskStatusPlanningFixture`,
  all 17 receiver tests, four parser/signature tests and the chapter-history
  proof passed. Every new status fixture has `execution: null`; these fixtures
  do not prove execution/actions or CLI success when both planning and execution
  are unavailable. Source still returned an error at that earlier boundary; the
  follow-through's public CLI proof now covers it.
- All 15 active OAuth tests passed together after the logging proof moved into
  an isolated process/subscriber. The ignored child entry is invoked by the
  normal test. This resolves the earlier grouped capture failure; no production
  credential logic changed and no retry masks it.
- Populated migration and five disposable installation-container checks passed.
  These are local/simulated-provider evidence. The full PM/repository gate, live
  Linear and the approved end-to-end command story remain unproven.
- Formatting, Clippy, migration checks and whitespace checks passed at the
  recorded boundaries. Swift emitted Ghostty missing-symbol warnings; the Linux
  installation build reported an unused variable in `controller/wave/metrics.rs`.
  Those warnings are retained limits, not new failures in this reconciliation.

Review corrected invalidation repair based solely on equal acquisition time and
positive fenced responses masquerading as absence. The chapter proof rejected
an older expectation that stale snapshots restore predecessor membership: current
membership stays unchanged and historical Tasks are read through chapter receipts.
A subsequent direct-cache assertion also failed because those entries had never
been acquired as normalized Task entities. It was not evidence of lost history.
The earlier ownership matrix reached Git authentication with an obsolete fixture;
its corrected conflicting Initiative ownership now fails before checkout.

At that earlier boundary, hard-stale/forced inspection refused failed refresh and
both planning readers hid invalidated bytes. The planning-evidence follow-through
above replaces those inspection limitations while preserving managed refusal. No restore/recreation semantics are inferred from the
permanent removal fence. Existing installation/Session continuity mechanisms are
useful discovery inputs, not LOO-298 agreement or independent runtime/store routing.

The current reconciliation also found a concrete counterexample to the documented
unknown-revision contract: a content-edit webhook without `updatedAt` manufactured
an empty revision and reached the timestamp parser before invalidation. The added
case failed with `invalid planning revision: the 'year' component could not be
parsed`. The parser now routes missing/empty revisions through the existing
planning-invalidation event, deferring ordered steering until a complete
observation. No new event type, storage or execution policy was needed.

Verification for that repair passed:
`cargo test -p loopflow --lib provider_revisions_and_webhooks_converge_without_execution`
(one convergence case, including invalidation, detail repair and no execution),
`cargo test -p loopflow --lib webhook::tests` (four tests), and
`cargo test -p loopflow --lib lfd::tests` (17 tests).
`cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and
`git diff --check` passed. Documentation checks confirmed the full acceptance
matrix is unchanged and local scratch links resolve. Broader recorded proofs
were not rerun. The plan, questions, research checkpoint and Infrastructure memory
now agree on completed freshness work, resolved OAuth capture and remaining gaps.
No live provider, execution migration, publication or Flow navigation occurred.

## Command experience: start with an idea

The primary walkthrough follows one fictional technical founder/CTO, Dave, who
can use the CLI directly but usually delegates mechanics to his agent. The
[full script](idea-to-task-command-story.md) selects commands through a single
invoice-export feature:

```sh
lf skill design                 # creates a design worktree automatically
lf skill launch-plan            # carries the design into one Task and starts it
lf task status APP-42           # the returned Task ID gives work a handle
lf task comment APP-42 "Use the current date filter for the export."
lf task run APP-42              # continue when the saved Flow needs resuming
# Only when an account blocks work:
lf auth status
# Once work is running and Dave wants visibility:
lf runs --active --task APP-42
lf usage --task APP-42
# After delivery reveals two continuing responsibilities:
lf wave status billing
lf wave status self-service
```

This is the target command experience, not a claim that automatic design placement
already works. `lf wt create invoice-export` is the explicit alternative first
step; design reuses a suitable existing worktree. The design session must make
its working context and next command usable without assuming a parent-shell
`cd`. Launch-plan verifies that the real design reaches the Task checkout before
launch; it does not restart planning or create competing execution.

Teach accounts and monitoring when work supplies a reason to inspect them.
The existing spellings are `auth`, `runs`, `usage` and `top`; this story does not
select namespace renames. Provider connection and store/runtime internals stay
behind ordinary Task commands. The same commands serve Dave and his agent.
The story ends after delivery: Dave asks for future work on billing accuracy and
customer self-service, then inspects their Wave plans. The script proposes
`lf wave create <name> --objective "..."` for missing owners, followed by ordinary
planning-only Task creation and Wave status. That creation surface is not present
in current source; its spelling/bootstrap semantics remain a product proposal.
Waves inherit the repository connection. Each owns a durable beneficiary/outcome,
a current plan and future Tasks; the original Task stays completed in its history.
Do not equate this scene with approval of a Wave-existence migration.

A companion cancellation case keeps `lf flow code "Prototype a printable invoice
summary"` usable in the checkout without reviving a canceled Task. It no longer
interrupts the successful story's ending.

## Intended experience

Task operations always use a local planning store. When a repository uses Linear,
Linear is the source of truth for what planning exists and sync maintains the
local representation. Without Linear, the local store owns planning. Connection
belongs to the repository; Waves do not select independent planning modes.

An explicit Task selector must resolve to a real planning record before work
starts. A branch, PR or old execution record cannot manufacture a current Task.
A fresh connected store refreshes from Linear before declaring a Task missing.
A network or permission failure means unresolved evidence, not confirmed absence.

Managed Task execution requires a valid Task and a matching plan. If the Task is
missing or execution no longer matches the plan, report it as invalid and stop
managed progression. Jack explicitly cut automatic reconciliation and continuing
mismatched Task execution from this change. Keep ordinary Flow execution available
in any worktree regardless of Task status; that execution does not advance the
invalid Task Flow.

Workers use the machine's official selected lf at each new step, unless explicitly
pinned. Selecting a new runtime must preserve the execution directory and exact
saved invocation. A release installed between steps is picked up at the next step.

## Sharing boundaries

Jack explicitly selected three product layers:

| Layer | What is shared | Owner |
|---|---|---|
| Open-source Loopflow without Linear | Wave goals, memory, Flows and Skills through the repository | Authored repository files; Git integrates changes. |
| Loopflow with Linear | Planning: the Wave's shared Tasks, priorities and progress | Linear; each local store is its synchronized representation. |
| Paid layer | Shared execution and coordination between participants | Outside this open-source Task's scope. |

Shared Flows and Skills are authored definitions. Sharing them does not share
captured invocations, cursors, claims, Runs or Sessions. Ordinary Git updates to
definitions do not rewrite already-captured execution.

Without Linear, each operator can maintain a private local plan and run Tasks.
Cloning or pulling the repo brings goals, memory, Flows and Skills; it does not
import someone else's backlog, Task status, claims, Runs or Sessions. Do not build
a Git-exported planning database or a Task-plan merge policy. Git handles these
authored files through its ordinary review/merge workflow; the local database
cannot silently overwrite those shared files during import.

Connecting Linear introduces shared planning. Any publication of an existing
private plan remains an explicit, previewed migration; authentication alone does
not publish it. A colleague can then discover the same provider planning without
inheriting another person's worker, execution ownership or account state.

Cross-Home discovery in LOO-334 finds one operator's existing work across their
known execution locations. It does not add teammate worker visibility, distributed
Task claiming, fleet scheduling or execution handoff. Linear Task status is shared
planning evidence, not cross-user execution authority. The paid execution layer
is neither implemented nor designed by this change.

## Original failure and surviving constraints

The reported LOO-298 incident had a Linear issue, PR #1296 and a checkout, but the
official CLI said no Task existed. Its record was in another installation's store.
The incident also reported a wrapper retaining older executable and data-directory
settings, divergent account/usage data, and a worker arriving after its caller had
withdrawn the claim at a ten-second deadline.

Baseline inspection at `a3820bf7e` established the following. The first three
defects are repaired by the planning slice; the runtime and startup defects remain:

- `ops/task.rs::task_status` required execution and could complete work from a
  merged PR. It now reads planning independently and observes PRs without completion.
- `ops/task_pm.rs::resolve_task_async` rejected uncached Wave membership before
  refresh. It now acquires the exact issue into the shared reader first.
- `pm/linear.rs::OwnedIssueNode::into_ownership` rejected Project-less issues.
  It now preserves existence and represents the missing relationship explicitly.
- Repository `engine/config.rs::PmConfig` already has provider and Linear Team
  fields. `ops/pm.rs::read_repository_team` reads them. Wave Initiative mapping
  can remain distinct from repository connection ownership.
- `engine/process.rs::resolve_current_home_lf_binary` can retain the current
  executable's installation rather than choose the machine's current selection.
  `ops/task_destination.rs` couples runtime and database selection and refuses
  differing local/installed Task IDs.
- `ops/task.rs::launch_task_process` releases its claim after startup-wait failure;
  `ops/child.rs::CHILD_STARTUP_GRACE` is ten seconds. A late child can become stale
  without another worker competing with it.

These repairs separate planning lookup from execution allocation. They do not
establish all managed-admission boundaries or justify arbitrary explicit Tasks.

## Accepted decisions

Jack subsequently requested a narrative in command selection: start with
`lf wt create` or `lf skill design` automatically creating a worktree, continue
through `lf skill launch-plan`, then introduce Task commands and later accounts
and monitoring. Jack suggested ending with Waves that organize future work around
two aspects of the initial feature. One composite persona/story replaces parallel
persona tours.
The linked walkthrough records the placement gaps and makes no shipment claim.


Jack placed the connection at repository level, then clarified: “if we are using
Linear as a source of truth it should be the source of truth for what planning
there is”. Jack subsequently selected one local store interface, synchronized to
Linear when available, and refusal of explicit Tasks absent from the applicable
planning store. The Apollo analogy describes the desired local-store simplicity;
it does not select a library, an offline mutation queue or a new sync service.

Jack also selected invalidation of execution that does not match the plan, and
continued ability to run an ordinary Flow in a worktree regardless of Task status.
These decisions remove the kickoff's proposal to keep progressing historical
Task execution through planning disagreement. Official worker runtime by default
and deliberate visible pinning remain the original Task direction.

## One planning store interface

Reshape existing planning storage and operations in place. Task commands, generic
Session Task selection, Wave views and the app use one local planning reader.
Provider synchronization supplies that reader; do not retain a competing direct
provider command path that bypasses the local model.

Use stable provider identity for synced records (repository/provider scope and
Linear issue UUID), with issue identifiers as aliases. Local Tasks have genuine
local identities, not fabricated Linear UUIDs. Preserve existing Task IDs and Run
foreign keys when attaching planning to execution. Planning can exist without a
checkout, invocation, claim or Session. Syncing an issue allocates planning only.

Use the existing PM cache and local planning machinery as the implementation
starting point, consolidating duplicate readers/writers. TaskSpace and TaskOps
remain conceptual planning/execution boundaries, not additional services or CLI
namespaces. LOO-298 owns the replacement execution schema; integrate with its
surviving Task owner rather than introducing a temporary second schema.

Normalize planning into entity records within the
existing SQLite store. Issue lookup, Wave listing, confirmed mutations and webhook
ingestion update the same Task by stable identity. Lists hold membership and order;
they do not own additional title/status copies. The draft migration now removes
serialized per-Wave snapshots and their payload consumers. `PmSnapshotRow`
remains a typed view assembled from normalized records, not another persisted
planning copy. This follows Apollo's shared-entity pattern;
it does not select Apollo as a dependency or require a general GraphQL cache.

Separate acquisition policy, stored freshness and managed Task validity. Missing
cached data requests a fetch; stale data requests refresh; a confirmed missing or
ineligible Task refuses managed work. Reuse bounded PM refresh mechanisms rather
than duplicating policies in CLI, workers and Swift. Offline managed execution
remains an open product choice.

### Research translated into implementation

The [source comparison](planning-store-sync-research.md) supplies the rationale
and evidence limits. These are the adopted target contracts; the checkpoint
distinguishes completed integration from remaining work:

| Research lesson | Loopflow contract | Verification |
|---|---|---|
| Apollo shares entities across queries | One planning record per stable Task identity; lists reference it. Detail, bulk sync, mutation and webhook use the same writer. | Detail/list order and mutation results agree without duplicate records. |
| Apollo separates storage from acquisition | Existing SQLite remains the local reader; repository connection selects the sync source. Reuse bounded refresh policy centrally. | The same reader works with and without Linear; empty connected stores fetch before absence. |
| Relay separates presence from freshness | Missing cached data, stale observations and invalid Task planning remain distinct. Cache eviction is never provider deletion. | Failed refresh retains dated data; partial lists never establish removal. |
| Apollo watches committed cache changes | App and CLI share the Rust store. Existing view refresh/subscription paths consume committed changes. | Task detail and Wave views agree after sync; Swift has no independent planning writer. |
| Apollo distinguishes optimistic from confirmed state | Connected mutations govern execution only after provider confirmation and local ingestion. | Rejected/pending Task creation cannot launch managed work. |
| Realm and PowerSync require additional offline write semantics | No durable offline mutation queue, automatic conflict merging or new sync service in this scope. | Failed writes remain failed/pending attempts, never successful planning or launch authority. |

Keep entity facts and query coverage separate: a fetched page establishes its
returned items, not the absence of all others. Apply returned fields without
clearing values omitted by partial responses. Prefer explicit domain updates or
complete entity refreshes to a generic GraphQL field-merging framework. Reuse
provider revision/order evidence to avoid old observations overwriting newer
facts; where ordering is unknown, refetch instead of declaring a winner.

Use existing webhook ingestion as an update/invalidation input and bounded fetch
as repair. Neither webhook arrival nor paginated API traversal is a transactional
server checkpoint. Do not reproduce PowerSync's checkpoint protocol without a
provider contract supporting it. Ordinary reads may reuse fresh local data;
explicit refresh and misses acquire provider facts before the shared reader.
Managed admission remains distinct from reading cached data.

### Repository connection and sync

Reuse repository `.lf/config.yaml` PM configuration for provider and Team scope.
Move connection controls/documentation to the repository owner. Wave Initiative
bindings map Waves into the provider hierarchy; a missing mapping cannot make
one Wave fall back to local authority inside a connected repository.

Connected planning represents Linear's Tasks, membership, title, status and
ordering. Local execution facts do not compete with those fields. Sync refreshes
planning through narrow writes that cannot accidentally overwrite claims, Runs
or invocation state. Do not describe this as replacing a separately authored
connected plan: Linear defines that plan.

On an explicit lookup, reuse a sufficiently fresh local record or refresh the
requested issue according to the shared acquisition policy. A connected cache
miss must attempt the existing direct issue query inside sync before reporting
absence. Do not require a preexisting
PM snapshot, local execution row or chapter receipt. Commit the observation to
the local planning store, then use the shared reader. Use existing bulk sync,
webhook and freshness paths for lists; avoid one network request per rendered row.
Keep the last successful observation's timestamp on failed refresh.

An issue with missing Project or unresolved Wave membership still has a planning
record and can be inspected. Managed Task launch needs enough valid planning to
supply its work and ownership; missing relationships report that limitation.
Only repository-bound Initiative associations participate in Wave mapping.

A provider outage does not disconnect the repository. Malformed or missing
connection configuration in a checkout previously known as connected is a
configuration discrepancy, not permission to author independent local planning.
Repository definitions establish the Wave set; connection bootstrap details remain open.

### Wave mapping selected; existence and migration details under review

Jack's mapping concern includes hundreds of provider objects. The
[scale findings](wave-existence-and-linear-migration.md#initiative-size-limits--verified-boundary)
find no published Project/Initiative count cap; actual capacity remains unverified.
Jack clarified that outgoing chapter Projects must be completed so they leave
the active plan. Chapter rollover transfers started unfinished Tasks with their
identity intact, settles backlog under the existing cancellation policy, then
completes the predecessor Project and retains its history. Completion closes the
chapter; it does not claim that every KR succeeded or complete transferred Tasks.
Keep the existing archival behavior after closure. `ops/chapter.rs` now completes
predecessors before archival through the same operation; its focused stateful
proof does not establish configured Linear acceptance.

For 100 Waves over 12 chapters, the settled result is 100 current Projects and
1,100 historical Projects, not 1,200 active Projects. Current-plan reads must
exclude completed/archived predecessors and load history only when requested.
Retain complete pagination and incremental refresh. No documented evidence says
completion removes an object from any provider storage quota; the accepted
benefit is a bounded active plan under either Initiative mapping.

Jack selected one Initiative per Wave, including subwaves, after considering
one for the repository. Each Initiative groups that Wave's current and completed
chapter Projects. The [comparison](wave-existence-and-linear-migration.md#alternative-one-initiative-for-the-repository)
retains the alternative as decision context, not an implementation option.

Use path names consistently: `A`, `A/B`, `A/B/C`. Each path names a distinct Wave
and Initiative. When the Linear account supports sub-initiatives, also set the
corresponding native parent relationship. Without that feature, the same names
and Loopflow hierarchy work with flat Linear Initiatives. Native hierarchy is
optional enrichment, not a prerequisite or a different Wave tree. Interpret
Jack's “parent status” as the parent relationship, not lifecycle status. Keep
stable provider UUIDs across renames; naming does not replace identity.

Prove slash-name creation, lookup, hierarchy and retry on accounts with and
without the feature. Provider depth limits must not restrict Loopflow's path
hierarchy. Missing capability is distinct from permission, network or mutation
failure; report those failures instead of claiming a native link succeeded.
Keep direct Project membership separate from descendant aggregation. Existing
company names or multiple native parents need adoption rules; this decision
does not authorize rewriting them. Discovery and migration details remain open.

Jack then steered the design toward creating Waves in Loopflow and publishing
or linking them to Linear, rather than deriving all Waves from workspace
Initiatives. This supersedes the earlier automatic Initiative-is-Wave proposal.
The [connection design](wave-existence-and-linear-migration.md) follows that direction:

- Repository-authored definitions establish Waves in both local and connected
  mode. Git shares goals, memory, Flows, Skills and stable Initiative bindings.
  A fresh clone sees the same Wave set and reuses those bindings.
- Linear owns shared chapter/Task planning for connected Waves. The local store
  syncs their mapped provider records. Reading unrelated Initiatives may suggest
  link/import candidates; it must not add Waves automatically.
- Creating a Wave while connected creates its corresponding Initiative through
  the explicit create operation, or links an explicitly selected existing one.
  Reads, login and Git pulls do not implicitly publish or duplicate Initiatives.
  Preserve an unsuccessful publication as incomplete setup, not a successful
  shared plan or a fallback independent local plan.
- Connecting existing local Waves previews link/create and Project/Task mappings.
  Reuse provider UUIDs on retry and across clones; names are not identity.
  Importing an existing company Initiative is an explicit operation, not automatic
  workspace adoption. Do not rewrite its existing planning to force a chapter.
- Linear remains authoritative for connected Tasks, their status and membership;
  repository authority over Wave definitions does not make a Git Task-plan replica.
  Unavailable/missing mapped Initiatives are connection discrepancies, not grounds
  to delete Wave definitions or recreate provider objects during reads.
- Migration transfers planning references, never execution authority. Invalid Task
  execution stays invalid; ordinary worktree Flows remain available.

Exact connection/import controls, existing Project selection, disconnect and
Wave deletion semantics remain open. The mapping direction does not authorize
provider writes in this review or implicit publication of existing private plans.

### Wave definitions follow worktree context

Jack clarified that the local store owns Loopflow's Wave view, importing definitions
from the repository. With an owning worktree, use its complete current files,
including uncommitted additions, edits and deletions. Without an owning worktree,
use the configured remote-main definition (normally the last fetched `origin/main`).
Jack proposed this baseline after ruling out dirty main as definition input. The
[context contract and proof](worktree-wave-definitions.md)
specify the conceptual resolver `waves(repo, worktree=None)`.

Do not merge main's Wave set into a branch view: a deleted Wave must stay absent
there. Keep main and other worktree answers independent even when one store serves
them. Refresh imports before returning API results; no manual sync or commit is
needed. Preserve stable Wave identities, provider mappings and shared planning;
contextual definitions do not create a separate Task-plan writer per branch.
All Wave lists, detail and selection use the same resolved context. Invalid or
unreadable files are not an empty successful import or a fallback to main.

A branch deletion changes that view immediately but does not delete provider
planning, historical execution or another worktree's Wave. Linear sync cannot
restore a Wave absent from the selected repository view. Ordinary worktree Flows
remain available. The proposed shared baseline uses the remote-tracking commit,
excluding unpublished local main commits and dirty main files. Ordinary reads
use the last fetched ref rather than claiming live remote freshness. Main should
stay clean; reads do not perform cleanup. Local-only repositories with no remote
baseline, explicit main-checkout context and refresh cadence need final policy.
Outward synchronization of edited mapped definitions remains open; API reads do
not imply provider writes.

### Planning writes

Local repositories create/edit/complete/delete Tasks in the same planning store
without calling Linear. Extend Linear-only Task and Project reference types so
local records use real local identities. Keep the one-current-Project invariant
and reuse existing Wave setup/chapter operations; no fake provider snapshots.

Connected writes go to Linear and update the local representation from confirmed
results. An attempted write is not current planning truth. Retain existing
idempotent creation markers; after an ambiguous response, inspect remote state
before retrying. Do not add a general offline mutation queue in this change.

Task creation must not require rotating a chapter just to populate a local
receipt. Resolve an existing, uniquely identified provider Project; if genuinely
ambiguous, show candidates and allow explicit selection for that operation.
Do not infer a chapter from an In Progress label or rewrite chapter history.

## Explicit Tasks and ordinary Flows

| Situation | Behavior |
|---|---|
| Connected issue exists, local planning is empty | Sync it into planning, then resolve normally. Do not allocate execution during status. |
| Explicit Task absent after conclusive resolution | Refuse managed Task work with a clear missing-Task error. Do not invent a Task from a branch or PR. |
| Linear unavailable, uncached selector | Report unable to resolve; do not launch or claim confirmed absence. |
| Task or execution attachment no longer matches the plan | Report invalid and stop managed progression. No automatic reparenting, reconciliation or historical-plan continuation. |
| Canceled/completed Task with a checkout | Report planning status; do not automatically reopen it or launch managed progression. |
| Ordinary Flow requested in that worktree | Run it without requiring a valid Task or settling the Task's managed invocation. |
| GitHub unavailable | Keep planning visible, with unavailable/dated PR evidence. |

Validate at managed launch/resume and subsequent worker boundaries so continued
Task work cannot bypass current planning validity. Use existing authority and
claim settlement; do not create another watchdog or cancellation ledger. This
Task does not implement proactive termination of an already-running provider
when remote planning changes. Invalidation must not advance the managed cursor.

Ordinary Flow selection must bypass implicit Task launch routing when a checkout
has an invalid or terminal Task. Retained attribution may remain historical where
supported; it grants no Task advancement authority. Cover both CLI and generic
Session entry points, rather than merely adding a new escape flag.

No cleanup or deletion of authored files/history is requested. Leaving bytes
untouched does not require recovery UI or make an invalid execution eligible.
The exact treatment of cached planning during an outage remains a review question.

## Cross-Home discovery

Planning and execution have different placement needs. Connected stores can each
sync the same Linear planning without creating duplicate workers. Local-mode
planning stays with its owning store. Locate existing execution before allocating
new execution. Ordinary status, run and Session review continuation follow the
same Task automatically after installation changes; they must not require database
paths, environment variables or manual repair. Diagnostic details can identify
locations, but database location is never Task identity.

Build transient candidates from the selected directory, existing installation
selections/retained receipts, known Home routes and Loopflow's known development
layout, including the legacy standard store. Reuse `known_installations` and the
minimal read-only Work identity reader. No machine-wide Task registry, recursive
search of arbitrary directories, copied credentials or foreign-store migration.
Canonicalize aliases and deduplicate by file identity: copied stores can share a
Home ID. Report uninspected locations and the bounded search scope.

Inspect minimal identity/location facts without requiring complete historical
parents. Unsupported schemas are uninspected, not empty. Detailed reads use a
compatible executable at the resolved location. Internal dispatch addresses
executable, `LF_HOME` and `LF_DB_PATH` explicitly; remote paths use existing
transport. This routing happens behind ordinary Task and Session commands.
Recommend a retained historical pair only when its artifact/store is verified.

If multiple divergent records exist, show locations and require explicit selection
for ambiguous writes. Do not merge them or pick the newest. Location selection
cannot make planning-invalid execution valid. A local Task found elsewhere can
be reached in its owning store without importing it.

Git/GitHub supply branch and PR evidence, never substitute planning records.
Associate exact recorded repository/branch or explicit PR Task links; issue-like
branch names are candidates only. Preserve unpushed refs and distinguish merged
history from an active successor. `task status` may sync planning and observe PRs,
but must not complete a Task as a side effect.

## Official worker runtime

At every worker boundary, resolve the machine's selected installation, independently
of execution-store placement. Official does not mean an HTTP check for the newest
release. Inherited `LF_BIN`, PATH and the parent's retained installation do not
constitute deliberate pins.

Capture one verified artifact/digest for each child and record it in existing
Session/Exec evidence under LOO-298's received contract. Keep that process's
executable stable; choose again at the next boundary. Same-Session tool wrappers
use that Session's executable/store pair. Preserve exact
invocation, claim and pending review while changing runtime bytes.

Proposed controls in existing Task run options:

```sh
lf task run DEM-334
lf task run DEM-334 --lf-bin /absolute/lf
lf task run DEM-334 --official-lf
lf task status DEM-334 --json
```

Pin/clear flags are mutually exclusive and persist on the stable Task owner
without replacing its Flow or depending on runtime child FlowSession identity.
A pin contains canonical artifact path/digest;
changed or missing bytes fail explicitly. Status distinguishes policy, pin,
last attempted runtime and next resolved runtime. Default migrated policy is
official; inherited environment never becomes a recorded pin.

An explicit lock is recursive: prepend a directory whose `lf` resolves to the
locked artifact to PATH for the locked process and every descendant. This includes
Flow children and bare `lf` commands in agent shells, not only direct worker execs.
Provider environment reconstruction must preserve that directory for Codex,
Claude and OpenCode. Preserve the remaining PATH so provider binaries and ordinary
tools remain reachable. Inherited LF_BIN/LF_CONTROL_BIN or an arbitrary ambient
PATH entry alone cannot manufacture explicit lock policy. A normal unlocked child
boundary reselects the official runtime; a lock remains until explicitly cleared.

Installed startup must honor an explicitly addressed compatible execution store
independently of its default store. Share path canonicalization with launch.
Retain branch-source isolation: arbitrary source binaries cannot migrate the
installed host store. Unknown private drafts remain a compatibility failure,
with pending work preserved; no downgrade or silent database switch.

Without a selected installation, source execution may use its private store and
invoking runtime, visibly reported. When an installation exists, stale wrappers
cannot force that fallback. Account and quota consolidation remain out of scope.

## Slow startup

After the bounded startup observation deadline, report starting and retain the
exact claim. Release on proven spawn failure or process death using existing
process fencing, not elapsed time. Concurrent retry observes the same child; a
late worker can publish under the retained claim. Proven-death replacement must
still reject stale late children. Coordinate this boundary with LOO-298's Session/Exec
ownership rather than building a parallel liveness mechanism.

## Integration and deletion path

The command walkthrough additionally requires verifying design auto-placement,
reuse of an explicit worktree, session working-context continuity and launch-plan's
artifact handoff. Current `wt create` requires a name, and current launch-plan
cannot adopt arbitrary unbound implementation checkouts. Implement or explicitly
resolve these gaps before demonstrating the opening sequence; do not manufacture
a second design or Task to make the story appear continuous.

1. Finish convergence on the normalized reader already implemented for detail,
   Wave lists and PM operations. Payload storage and its consumers are removed;
   incomplete ownership is representable and lookup refreshes before resolving
   membership. Issue webhooks now feed shared invalidation and provider revisions
   order Task facts. Project fact ordering and public invalid-state evidence now
   follow the same owner. Finish relationship-specific revision acquisition and
   repair of unresolved membership; acquisition timestamps cannot settle it. Preserve
   migration history and the existing deletion receipts. No second planning copy.
2. Adapt Task/Project identities and planning writes for local operation. Preserve
   IDs and execution through forward migration; coordinate with LOO-298 first.
3. Update Task operations, direct `--as task:` resolution, Session selection,
   Wave views and Swift DTO/actions together. Represent optional execution and
   planning freshness explicitly, without fabricated idle/worktree defaults.
4. Enforce managed Task validity while keeping ordinary worktree Flows independent.
   Delete the kickoff's mismatch-continuation and automatic reparenting proposals.
5. Reuse location evidence for discovery and runtime/store routing. Replace the
   local/installed Task-ID equality refusal with actual resolution and selection.
6. Persist runtime policy on the surviving execution owner and repair startup
   admission. Update CLI, planning/Homes docs and TESTING.md alongside consumers.

LOO-298's current contract arrived through the authorized read-only contribution
summarized above; the kickoff's unregistered failure is historical. Integrate
against its stable Task, AgentSession, FlowSession and Exec owners, preserving
both migration frontiers. Do not edit its branch, repair auth, copy stores or add
policy to runtime child FlowSessions scheduled for removal. Use `lf rebase` when
integrated work exists. Coordination success does not establish integrated bytes
or waive the public continuity proof.

## Proof and finish line

Extend `scripts/test_task_installation.py` and existing real-CLI fixture support.
Use a disposable OS account/container with two stores, a Git repository/local bare
remote, simulated Linear/GitHub and distinguishable installed artifacts. Mount no
host installation or credentials. Provider simulation is not configured live proof.

1. A owns execution; B starts without planning snapshots or a chapter. Public
   status by identifier/UUID in both syncs the same provider Task, finds its exact
   branch/PR and locates A. Ordinary Task/Session commands route there automatically
   without user-supplied database settings. B gains planning, not a second
   worker/invocation. Source-private lookup does not modify foreign DB/WAL bytes.
2. Planning-only and Project-less issues are stored and inspectable. Unresolved
   relationships prevent only operations requiring them. Permission denial,
   timeout, partial GraphQL errors, archive omission and confirmed removal remain
   distinguishable; pagination omission is never conclusive deletion.
3. Two Waves share repository authority. Missing one Initiative mapping never
   enables local fallback. In an unconnected repository, exercise real local plan
   setup and Task create/edit/run/complete without any Linear requests. Discover
   its owning store from B; same-name local Tasks do not collapse.
4. Fresh Linear changes control stored title/status/order/membership. Confirmed
   absent Tasks and mismatched execution refuse Task run/resume/next-step launch
   without creating a claim or advancing the cursor. A nonexistent explicit
   selector creates nothing. Ambiguous provider write retries neither duplicate
   Tasks nor overwrite newer planning.
5. In the very same checkout, run an ordinary Flow with missing, canceled or
   mismatched Task planning. Prove execution reaches its intended work without
   resuming or settling the invalid Task invocation. Merely accepting CLI syntax
   is insufficient. Cover the generic Session selection path too.
6. Run two actual worker boundaries. Select R2 through normal disposable install
   between them; the next worker consumes its claim with R2's digest and unchanged
   invocation/execution directory/review. Poison PATH and inherited LF_BIN and
   LF_CONTROL_BIN with R1 so the original defect would fail the proof.
7. Pin R1, change official selection and prove execution/status retain the pin.
   Clear it and prove R2 runs next. Modified/missing pin and incompatible store
   preserve the pending boundary and report failure.
   Verify recursive PATH resolution through Codex, Claude and OpenCode agent shells
   and nested `lf` children. The locked directory precedes a conflicting official
   installation on PATH, without hiding unrelated tools. A provider-launch env map
   or successful direct exec alone does not prove agent shell behavior.
8. Delay an actual child beyond ten seconds. The caller reports starting and the
   late child consumes the same claim. Concurrent retry launches no duplicate.
   Separate proven-death/replacement evidence rejects a stale child.
9. Divergent execution copies, unreadable stores and filesystem aliases produce
   honest locations without migration or timestamp takeover. Equal Home IDs do
   not collapse distinct stores. Invalid execution remains invalid after routing.
10. Rust/Swift DTO fixtures and Task actions agree on planning-only, unavailable,
    invalid, terminal-with-checkout and pinned states. Migration tests preserve
    populated historical IDs and execution records. Status never completes work.
11. List-then-detail and detail-then-list share one planning identity. Confirmed
    mutations update both views. Partial responses/lists cannot clear unrelated
    fields or imply deletion, and older observations cannot resurrect confirmed
    removed planning. Failed refresh preserves last-good data and observation age.

12. Replay the linked command story from design to launch-plan to one Task, then
    status/steering/continuation and ordinary Flow after Task invalidation. Prove
    auto-created and explicitly created worktrees converge on the same design
    handoff. Accounts/monitoring enter only at their story moments. The final Wave
    scene plans two future outcomes without launching or duplicating the completed
    Task. Proposed Wave creation/bootstrap must be resolved before claiming that
    scene works; its inclusion does not establish current implementation scope.
    Source checks alone are not an end-to-end demonstration.

13. Two independent local-only users of one repository receive the same committed
    Wave goals, memory, Flows and Skills and retain separate private plans and execution. Pulling
    goal, memory, Flow and Skill definitions updates imports no Tasks, worker claims or history. With Linear,
    both can read the shared plan, but no execution control transfers with it.

14. Prove [worktree-specific Wave imports](worktree-wave-definitions.md): main and
    two worktrees share one local store yet return their own definitions. Dirty
    add/delete/edit and revert affect the owning view immediately; read order,
    branch switches and Linear refresh cannot overwrite another view or resurrect
    locally removed Waves. Context-free reads use the remote-main baseline.
    No provider mutation or execution allocation follows from definition import.

15. Replay Jack's LOO-334/LOO-298 installation-switch incident in disposable
    locations: planning and execution exist before the switch; a design review is
    pending. After selecting another installation, ordinary `task status`,
    `task run` and Session review continuation find the same Task, checkout and
    exact review. Preserve claims, captured Flow, attribution and history, launch
    no duplicate worker, and use the selected runtime at the next child boundary.
    Include an obsolete saved `open_argv` that would reject `session` as a skill.
    No manual database paths, environment repair or owning-store selection counts
    as a pass. Genuine divergent execution conflicts remain explicit.

Primary end-to-end command after extension:
`uv run python scripts/test_task_installation.py`. Use one focused behavioral proof
per changed boundary; affected suites once at gate. Rust changes require
`cargo fmt` and `cargo clippy --all-targets -- -D warnings`. The checkpoint above
records the partial implementation's proofs; this full matrix remains unproven.

## Scope and implementation handoff

In scope: repository connection, one synced planning store, explicit Task validity,
local lifecycle, worktree-sensitive Wave imports with a remote-main baseline,
per-Wave Initiative mapping and chapter closure, cross-Home location, independent
ordinary Flows, official runtime and pins, slow startup, consumers and public-CLI proof. Internal slices do not
individually meet the full Task's finish line.

Out of scope: shared execution between participants (the paid layer); Git export,
import or merging of private Task plans; reconciling execution that disagrees with
planning; continuing an
invalid Task Flow; merging stores/accounts/quotas; a new sync service or offline
mutation queue; remote fleet discovery; replacing LOO-298's execution schema;
Wave deletion policy; automatic chapter rotation; host promotion.

Current first slice now refreshes an explicit connected issue into local planning
and reads it through the shared store without allocating execution. Focused proofs
cover a missing Project, missing selector and unavailable provider. Managed
validity and ordinary Flow independence still require the remaining integration. Jack approved proceeding with this
architecture. The cached-Task outage policy and connection-transition semantics
remain explicit in questions.md; they must not be silently inferred from library
behavior. They do not prevent the first slice. No Flow navigation is selected here.

## Retained observations and evidence limits

- At kickoff, official `/Users/jack/.local/bin/lf task status LOO-298 --json`
  returned `Error: no Task exists for "LOO-298"`. Ambient help exposed an older
  command surface; the official entrypoint exposed current Task/Session commands.
- The first coordination request used an invalid untyped selector. The corrected
  `--as task:LOO-298` request failed before launch; no reply established the current
  execution-schema contract. No LOO-298 branch was edited.
- Earlier source inspection and Linear documentation supported direct issue
  lookup, partial GraphQL errors and archive exclusion from default lists:
  [GraphQL API](https://linear.app/developers/graphql) and
  [rate limiting](https://linear.app/developers/rate-limiting). These are retained
  kickoff findings, not fresh provider verification or evidence of live payloads.
- Kickoff review caught Home-ID aliasing, Project-required existence, runtime/store
  coupling and Linear-only Project identity under local creation. Those constraints
  remain; the direct-read architecture and mismatch-continuation proposal do not.
- The interactive design review performed no implementation, live provider
  request, migration, publication, promotion or acceptance demonstration.
  Subsequent implementation and local proof are recorded in the checkpoint above.
