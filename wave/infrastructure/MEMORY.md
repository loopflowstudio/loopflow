# infrastructure wave memory

Renamed from `systems` in the 2026-07-08 wave/project/task restructure. Owns dependable self-hosting, verified releases, and architecture minimalism. The configured release schedule and accepted proof obligations govern current work; older nightly/weekly notes below are historical.

Release-specific findings live in [release memory](release/MEMORY.md).

## Branch data and command ownership (LOO-321, branch evidence 2026-09-28)

[Branch data isolation · LOO-321](https://linear.app/loopflow/issue/LOO-321)
addresses the installed-database incident recorded under LOO-305 below;
[Branch data and command cleanup · PR #1308](https://github.com/loopflowstudio/loopflow/pull/1308)
carries the implementation. The retained
[design and proof ledger](https://github.com/loopflowstudio/loopflow/blob/5f10576bd8e060e20c6aeb9addca8a462bc838b6/scratch/branch-build-own-home.md),
[Jack's review decisions](https://github.com/loopflowstudio/loopflow/blob/5f10576bd8e060e20c6aeb9addca8a462bc838b6/scratch/branch-home-interactive-demo.md),
and [demo handoff](https://github.com/loopflowstudio/loopflow/blob/5f10576bd8e060e20c6aeb9addca8a462bc838b6/scratch/branch-build-demo.md)
preserve local history before scratch clearing; remote availability was not checked.
Current mechanics belong in [CLI docs](../../docs/lf.md) and
[Homes and processes](../../docs/architecture/homes.md).

- **Home means execution destination.** Jack retained machine placement and the
  need to reach Mac mini work through the same app. `LF_HOME` selects a data
  directory. A branch snapshot preserves Home IDs and placement without creating
  another Home or acquiring process authority. The earlier local-only proposal
  is superseded; removing resident Wave controls does not remove remote placement.
- **Installation receipts own stores.** Source execution redirects inherited
  installation-owned data to a source-specific directory and seeds it once.
  Retained stores and filesystem aliases remain owned; repeated commands preserve
  private writes. Explicit private data overrides stale control pins. Foreign
  execution context is cleared, while Task origin survives only as an installation
  restriction. Tasks cannot promote themselves; read-only preflight remains usable.
- **Move the whole managed operation before effects.** A selected installation
  supplies both executable and database before `create --run`, Flow changes or claims.
  Return its Task snapshot instead of rereading branch data. Without an installation,
  source execution uses private data; the earlier installed-only prerequisite is
  superseded. Exact review completion consumes installed readiness and feedback,
  never copied readiness. Private and installed writes do not synchronize, and
  branch-only Task identity is not transferred implicitly. Database isolation
  does not isolate provider mutations or shared checkout edits.
- **Continue saved work; replace it explicitly.** Jack selected `task run` for
  start and continuation, retaining captured definitions, review waits and failed
  decision feedback. Retry revokes pending merge intent; `task restart` explicitly
  replaces the invocation. `task checkout` allocates without execution and restores
  a missing checkout from retained Task/PR/branch identity. Dirty invoking or
  canonical checkouts are valid; occupied paths, other owners and branch history
  remain protected. A failed first allocation can retry its pinned base.
- **Remove controls across their consumers.** Task/Wave enablement and Wave
  start/stop/pause/resume/serve are removed from CLI, runtime, app and current DTOs.
  Released enablement columns and old paused frontmatter are inert historical
  bytes. Chat connects automatically through the local daemon without moving
  remote placement; schedules survive. Reads now belong to `catalog`, `wave list`,
  `wave status`, `wave probe`, and `pr checks`, without aliases. Other command-catalog
  proposals remain unselected, including PR viewing/abandonment and cron editing.
- **Recovery preserves evidence.** Incompatible-data diagnostics name the database
  and applied draft names, IDs and checksums. A retained executable/database pair
  is advice only after artifact verification and exact-store read-only preflight;
  missing evidence stays explicit. Returning to another preserved directory does
  not repair or merge private writes. Published promotion reports both database
  paths and the retained installation ID; it does not transfer Task history.

Recorded local proofs cover snapshot preservation, ownership, checkout recovery,
saved continuation, command/DTO agreement and fixture chat. All four disposable
Linux installation proofs passed after Docker recovered, including inactive
retained-pair recovery and installed readiness/agent persistence; the stranded
fixture container was removed. These supersede the earlier unrun-recovery and
cleanup gaps. The harness uses authored installation records and current binaries,
not Jack's installation. Fixture chat uses simulated tmux/provider executables
and two concurrent callers; the earlier twenty-caller deadline failure remains
outside that proof. Final compression records focused Rust/Swift and static passes.
This memory curation reruns no behavioral suite and establishes no hosted CI result.

The real installed-Session/new-draft Task-write and configured installed-worker
demonstrations remain unproven. A prepared review Run is not worker execution;
the worker proof needs executable, data directory, Home, claim and Run evidence
under stale branch pins. Jack has not accepted the new app behavior. Remote app
chat transport is not implemented here. Seeded usable demo context, private-draft
repair, redirected-daemon children, local OS death evidence and demo write-back
remain deferred; the reported Linux oversized-prompt defect is outside this scope.
Readiness, fixture success and publication authorize neither demo acceptance nor
Flow navigation, Task completion, provider mutation or installation promotion.

## Account auth consolidation (LOO-320, branch evidence 2026-09-27)

[Make account login, usage, and auth output clear and reliable · LOO-320](https://linear.app/loopflow/issue/LOO-320)
owns [Account auth consolidation · PR #1307](https://github.com/loopflowstudio/loopflow/pull/1307).
Jack Heart approved the delivered scope at demo and directed compress →
update-wave → gate, then queue and land. Cross-account Session continuation and
native credential refresh coordination are outside this Task and require separate
Tasks; their follow-up identities are not established by this curation. Ranking
new Sessions by remaining headroom also remains out of scope.

The [approved design and scope cut](https://github.com/loopflowstudio/loopflow/blob/1891b5ea649c3c21780c264fa83ba5b00452045c/scratch/make-account-login-usage-and.md),
[demo, approval and final compression proof](https://github.com/loopflowstudio/loopflow/blob/1891b5ea649c3c21780c264fa83ba5b00452045c/scratch/account-demo.md),
[configured usage failure](https://github.com/loopflowstudio/loopflow/blob/1891b5ea649c3c21780c264fa83ba5b00452045c/scratch/account-login-slice-2.md)
and [native refresh investigation](https://github.com/loopflowstudio/loopflow/blob/1891b5ea649c3c21780c264fa83ba5b00452045c/scratch/account-refresh-boundary.md)
retain detailed evidence in local history before scratch clearing. Remote
availability was not checked. These are branch results, not shipment.

- **Reduce commands while preserving useful objects.** Six auth leaves replace
  separate profile/access/import/configure/reset and top-level route commands,
  without aliases. Reusable named Chrome profiles remain; managed accounts and
  local services, including Linear, remember independent ordered bindings.
  First-time selection is remembered after success; a last-used browser window
  is never the fallback. Full-email onboarding stages identity before registration.
  Current mechanics belong in [subscriptions documentation](../../docs/subscriptions.md).
- **The browser handoff and printed URL are different evidence.** Claude Code
  2.1.283 emitted an OSC 8 manual URL before handing a callback URL to its browser
  helper; both had `code=true`. Parsing OSC alone cannot choose native completion.
  The provider owns OAuth/callback; the existing flow handle owns the private
  FIFO, child/readers and cancellable manual input. An input pipe proves manual
  support, not a requirement to wait for a code. Cached active auth cannot prove
  that the current authorization attempt or its persistence succeeded.
- **Read-only inspection includes its dependencies.** Cached status must avoid
  provider launch, token decryption/import, broker contact and directory creation.
  An inherited lease has no identity catalog: report uninspected forwarding
  instead of inventing account rows or contacting its origin. One transient report
  drives text/JSON; managed presence, local token metadata and server acceptance
  remain distinct. Cron consumes accepted managed JSON evidence, not exit zero.
- **Observations retain their own age and owner.** Persist returned windows
  transactionally before rendering; omitted windows retain source/time and never
  become fresh or zero. A passed reset proves no new capacity. Verification uses
  a narrow credential-state writer and preserves routing/cooldown; clearing local
  cooldown preserves windows. Cached Claude plan metadata is not a live plan.
  Rejection and acceptance from replaced credentials are discarded, but file
  comparisons do not exclude native refresh or the final compare/write race.
- **Selected account precedes Session discovery.** Record the actual selection
  per attempt in existing Run events, then attach it when the provider Session
  ID arrives. Retries preserve earlier attribution; an ambient retry cannot
  inherit the previous managed account. Requested identity is not selected
  identity. Changing homes alone cannot prove cross-account conversation history.

Four live proofs remain unproven despite delivery approval: browser login without
pasting a code, first-time managed connection, remembered Linear profile targeting,
and real Claude/Codex usage windows. The configured Claude probe returned
`invalid_grant` and obtained no usage payload; the retained `limits[]` decoder
fixture remains synthetic. Native refresh research found provider-owned directory
locks and conditional secure-storage writes, including macOS Keychain; Loopflow's
flock and atomic file replacement do not establish interoperability.

The copied-Home demo exposed two retained limitations: `status --details` failed
before the copy received `auth_browser_bindings`, and route inspection reported
stored connected evidence while cached status detected the missing credential.
Neither was selected as new implementation work at approval. A database copy
still points at original managed credential homes, so it does not isolate live
verification. Jack forbids running this draft-bearing branch binary against the
installed Home. Use disposable Homes with inherited LF_* authority removed for
source proofs; installation remains owned by ordinary delivery.

Recorded evidence includes 328 focused library passes after rebase, the candidate
build and eleven empty-Home command checks, and 13 focused synthetic checks plus
formatting/Clippy after delivery compression. None establishes the four live
proofs, PTY secrecy/restoration, real manual fallback or installed acceptance.
Gate made the existing attempt-completion receiver public after finding that
removing event callbacks had left crate callers unable to consume the result.
The [Rust API migration](../../docs/subscriptions.md#rust-api-migration) documents
the source-breaking replacements; it does not claim external consumers migrated.
Repository-only caller searches cannot prove an exported API has no consumers.

The 2026-09-27 affected gate passed architecture, 250 Python tests, 2,127
materialized Rust tests (nine skipped), 78 website tests (three skipped),
formatting/Clippy, migration history, Ruff and shell syntax. Receipt
`20260928T023432Z-18082-33c4dca6` records the run. One telemetry fixture passed
after gate terminated its verified stalled SSH-fetch child; this is an intervened
local result, not unattended fetch proof. Fresh-Home runs/usage/doctor JSON checks
passed through the materialized candidate; doctor retained empty-history and
freshness warnings. The gate's source API edit preceded Rust checks and docs
preceded website checks, but the receipt's initial-tree fingerprint predates
those edits and is not reusable for the final tree. These results establish
neither installed acceptance nor the four live proofs. Swift, app/UI and slow
end-to-end checks remain with CI. No delivery or Task disposition follows from
this evidence alone.


## Task deletion and command ownership (LOO-305, branch evidence 2026-09-27)

[Task removal and command consolidation · LOO-305](https://linear.app/loopflow/issue/LOO-305)
owns [Task command consolidation · PR #1302](https://github.com/loopflowstudio/loopflow/pull/1302).
Jack's final scope is provider/local deletion, removal of `pm`/`work`, compression
and delivery through the saved Flow. Execution settlement remains deferred.
The accepted command map and detailed proofs survive in local commit
`4a14c0a47dc6e04be9668fb72b737828565a931d`:
[design](https://github.com/loopflowstudio/loopflow/blob/4a14c0a47dc6e04be9668fb72b737828565a931d/scratch/cancel-linear-issues-through-lf.md),
[implementation evidence](https://github.com/loopflowstudio/loopflow/blob/4a14c0a47dc6e04be9668fb72b737828565a931d/scratch/implementation-evidence.md),
[demo](https://github.com/loopflowstudio/loopflow/blob/4a14c0a47dc6e04be9668fb72b737828565a931d/scratch/task-deletion-installed-demo.md),
and [compression/rebase proof](https://github.com/loopflowstudio/loopflow/blob/4a14c0a47dc6e04be9668fb72b737828565a931d/scratch/deletion-namespace-compression.md).
Remote availability was not checked. These are branch results, not shipment.

- **Commands follow their objects.** Task owns create/status/edit/comment/run/
  complete/delete; Wave owns connection, sync and placement; repo owns reteam
  and webhooks; `doctor --planning` owns diagnostics. `pm`/`work` and Task
  abandon/recover have no aliases. Older command spellings below are historical.
  Current mechanics live in [CLI docs](../../docs/lf.md) and
  [planning architecture](../../docs/architecture/planning.md).
- **Identity, confirmation and outcome differ.** Observed issue identity survives
  refresh/chapter replacement but never authorizes a new provider mutation.
  Fresh ownership authorizes deletion; acknowledgement or explicit trash evidence
  confirms it. Missing membership proves neither. Manual confirmation atomically
  retires Ready Tasks and preserves terminal times, Done outcomes, PRs and Git.
  Chapter confirmation shares the insertion while retaining its own classification
  and receipt transaction. Stale snapshots cannot erase positive confirmation;
  old abandonment/applied receipts cannot manufacture it.
- **Prepare before filing.** Planning-only creation allocates no checkout or
  Task row and needs no agent account. Execution creation consumes validated
  placement, pinned base, Flow/auth and selected Task agent. Retry markers survive
  notes edits and reuse persisted identity/title. Post-create allocation failure
  remains recovery work; do not replace preflight with compensating deletion.
- **Completion and planning have separate writers.** Check provider terminal
  conflicts before new local Done, including the confirming refresh. Retain merged
  PR evidence for retries; narrow writes preserve refreshed planning and execution
  facts. Repeated completion preserves its original event/time.
- **Checkout identity is its own branch.** Upstream is tracking information and
  may name main or a stack parent. New branches disable automatic base tracking;
  publication establishes their own origin branch. Historical Run attribution
  resolves retained identity independently of launch eligibility. Blank participant
  overrides fall through to configured/Git names; markerless steers use the
  provider author without rewriting comments.
- **Removal is not termination.** A completed capture and released claim can
  leave a live Exec without an exact Task join. Native per-Run locking cannot
  establish Task-wide admission or landing settlement. The retained reproduction
  and deferred scope are in the design's linked evidence; complete Session/Run/
  activity disappearance and process settlement remain unproven.

The recorded Linux real-binary deletion proof used synthetic Linear and disposable
stores; macOS platform TLS ignored its child-only CA setting. TESTING.md owns
that fixture contract. The source demo deleted LOO-299–302 against configured
Linear, retried successfully and verified refreshed Wave/roadmap absence. It also
advanced installed-Home drafts and broke the older installed CLI. Jack's later
steer forbids branch-binary access to that Home and any branch promotion;
[Installation incident · LOO-321](https://linear.app/loopflow/issue/LOO-321) owns
recovery. The demo's promotion handoff is superseded. All further source proofs
use disposable Homes with inherited LF_* authority removed.

Rebase retained main's shortened Flow and Task-agent semantics. Compression moved
Task scenarios out of OAuth tests and removed duplicated setup/cases, without
counting moved lines as deleted. Recorded focused checks and Clippy passed;
no full gate or installed acceptance follows. This curation changes no chapter,
Task disposition or delivery state.

## Task convergence (LOO-319, branch evidence 2026-09-27)

[Make the default Task Flow converge, and let a Task choose its agent · LOO-319](https://linear.app/loopflow/issue/LOO-319)
owns this work; [Task convergence · PR #1301](https://github.com/loopflowstudio/loopflow/pull/1301)
carries the implementation.
The retained [slice evidence](https://github.com/loopflowstudio/loopflow/blob/3abb64bf1a25d4109a8353e22f4a55f664a6f4de/scratch/converge-flow-and-task-agent.md)
and [demo decisions](https://github.com/loopflowstudio/loopflow/blob/3abb64bf1a25d4109a8353e22f4a55f664a6f4de/scratch/task-convergence-demo.md)
preserve failed attempts, measured replacements and proof scope before scratch
clearing. These identify local history; remote availability was not checked.

Jack initially selected implement → compress → review-slice → loop-decide.
The September 28 realign branch supersedes that inner loop with implement →
compress → refresh → loop-decide; refresh composes rebase → realign. Publication
follows convergence before the existing human demo. Jack accepted refresh's
leased branch push. Realign edits the plan, code, and identified Wave memory;
its minimal evidence summary replaces the two-pass no-replacement blocker and
mandatory slice ledger. Loop-decide still judges caller-supplied criteria;
publication checks readiness because successful reconciliation can leave work
unfinished. The [prompting lessons](../intelligence/MEMORY.md#reconciliation-and-reusable-skills-branch-evidence-2026-09-28)
retain the rationale and agent-behavior proof limits. Concept-review stays a
self-contained interactive skill. Ship calls gate directly; task-gate is removed.
Captured invocations retain their definitions. Tests must locate decision policy
in the captured Flow rather than copy a catalog step index: shortening pursue
exposed exactly that stale assumption in CI. The repaired test retains both
missing-verdict rejection and interrupted-verdict removal; its two focused
Flow tests passed locally. This is not a new hosted CI result.
Catalog assertions must track both the authored and expanded Flow after removing
steps. Ordinary Flow review fixtures also need the shared ambient guard under
the environment lock: setting `LF_HOME` alone leaves Run reads bound to an
inherited `LF_CONTROL_HOME` during migration-materialized tests.

Task agent precedence is persisted Task choice → captured skill → checkout
configuration; the default agent remains unchanged. Failed decisions use the
existing blocker and keyed unblock Session. A live decision waiting on its Ask
retains its claim and reassesses in the same Run after completion; a failed Run
cannot regain authority and requires resume at the same decision. Match Task,
invocation, boundary and parent Run before projecting an Ask as Blocked. Feedback
does not supply a verdict. PM refresh and stale settlement cannot overwrite the
dedicated agent choice; the next launch reloads it, including human review.

Provider tool commands need both the intended executable and Home. The live
fixture disproved a PATH-only repair: development readers ignore control pins.
The shared harness environment writer forwards ordinary Home/DB overrides for
development and pins executable discovery only for uninstalled development,
preserving installed current-Home selection. Real Codex reached the fixture via
bare `lf`, opened one policy Ask and reassessed in the same Run after synthetic
feedback; this does not prove configured Claude or Jack's acceptance.

Run event/CPU observations project Stalled without granting signal authority.
Retain the first body PID/start identity across missing and replacement samples;
adopting a replacement later defeats PID-reuse protection. Unknown samples stay
unknown, active tool CPU prevents a stall, and live Ask takes precedence. Local
stall proofs use simulated elapsed history, including a real sleeping process;
they do not establish the configured five-minute observation. Free disk is the
resource gate; aggregate build bytes are measured only, and 24 GiB triggers
local cleanup. The sibling/low-disk/self-cleanup proofs use temporary filesystem
fixtures; another active worktree's oversized build only warns. TESTING.md owns
the operational resource and fixture-isolation rules.

Jack authorized landing through the Task Flow. The demo's direct landing was
withdrawn; subsequent delivery belongs to the Flow. Configured Claude launch and
resume, a real five-minute stall, and rendered desktop agreement remain unverified.
The retained real Codex policy proof predates the generic loop-decide revision;
it does not establish fresh policy execution. These gaps are not passing results
or proof of Task completion. Do not run LOO-305's branch binary against the installed
Home: Jack reported that its drafts broke installed `lf` (LOO-321). Use disposable
fixture stores for source verification; credential repair remains out of scope.

## Data model and performance decisions (2026-09-26)

Jack's rule, verbatim: "The main user objects should line up with the main
tables in the DB and when we see stuff like this where a main record is
actually a union over 4 things, we should be suspicious." The trigger was a
product-first review of Session/Run/Task/Wave: a Session is four read-time
projections over four stores (Run dir, `task_flow_positions`,
`human-sessions/*.json`, `flows/*/position.json`); Run→Task is a `task:`
string in a ranked subject list with a two-value source, mirrored into a
Task event because the Run cannot be queried by Task; every post-launch fact
(name, completion, attachment) became a sidecar beside the manifest; reads
reuse the launch resolver, so a bound Session turns into an orphan when its
Task's PR merges. The store's `runs` table has the Task FK and no writer.

The approved model belongs to LOO-298, outside the LOO-291 delivery. Rewrite
docs first as the spec, then tables/readers, then research what the new model
makes deletable and remove it. The
[decision history](https://github.com/loopflowstudio/loopflow/blob/be7a02db0/scratch/demo-native-workspace.md)
and [scope handoff](https://github.com/loopflowstudio/loopflow/blob/be7a02db0/scratch/deferred-work.md)
preserve Jack's approval and the follow-up split. These are target contracts,
not claims about tables already implemented:

- Repository has Chapters, a repo-wide clock incremented for every Wave at
  once; Project = (Wave, Chapter), unique, owns Tasks, KRs, metric targets and
  the Flow template its Tasks run by default; Task ⇒ Project ⇒ Wave.
- Task owns Flow invocations (0..n, one current) and may invoke any Flow
  ("trust our users"); the invocation records which. An invocation is one
  object: unrolled graph + cursor + per-loop return counts + nullable parent
  for runtime nesting only, never template composition (invocations are
  always fully unrolled). No step-occurrence object, no path-string node key,
  no "Flow position", no "recommended Flow".
- `runs`: nullable `invocation_id ⇒ task_id ⇒ wave_id`, constructor fills
  upward and refuses a mismatch; node + iteration tuple in an invocation;
  `work_source` declared|checkout|inherited|bound.
  A Run may have neither Task nor Wave, or a Wave alone. Task implies Wave;
  bind preserves invocation membership. The manifest remains launch evidence.
- `sessions` is a child of `runs` (run_id unique, kind, title + provenance,
  state, ready_summary). Bind = update the Run's task/wave; rename = update
  the title; started derived from Runs; bind allowed on done Tasks; usage
  follows the field.
  Ask and Flow review Sessions become rows keyed by their existing
  `session_run_id`, replacing the four-store projection.
- Every denormalization has a Pydantic-style validator or is deleted. No
  sidecars, no shim: a one-time migration fills the columns from old
  subjects and `session-name.json` and drops them ("hack my computer if need
  be, keep the codebase clean").
- Method for any model review: derive the user's objects and the APIs between
  them from the product first, then check the infra for hops.

Performance (instrumentation implemented in LOO-291; LOO-300 continues): `os_signpost`
intervals under `studio.loopflow`/`perf` for cold start, navigation, Wave/Task/
Session paint, every `lf` read, Markdown parse and terminal key-to-draw;
`scripts/benchmarks/desktop-performance/record_live.py` records local usage
without telemetry. The retained [90-second idle recording](../../scripts/benchmarks/desktop-performance/20260926-demo-app/report.md)
measured `session list` at p50 809 ms and `roadmap --all` at 3.49 s; `ps --json`
was 274 ms, so not every read exceeded the proposed 300 ms budget. It recorded
zero hitches but one 1.85 s potential hang and nearly flat RSS. The earlier
installed build's six-second probe measured 51 ms/s hitches; these different
windows/builds do not prove a causal improvement. Republishing identical readings
was found in source and removed; remaining hang causes need profiling.

LOO-300 owns Session streaming, projection caching and the density harness after
the data-model work. The handoff records passes only for cold-start-to-outline
and terminal-key-to-echo. `PerformanceCatalogueTests` also retains a filter test
that can skip when SwiftUI exposes no NSTextField; six other tests were removed
after mounted paint hooks failed to fire. Missing results remain proof gaps.
Key-to-next-draw and PTY echo are proxies, not glyph presentation. Click ≤100 ms,
`lf` read ≤300 ms off the main actor and idle ≤5 ms/s hitches remain proposed
targets until comparable measurements support published budgets.

S5 currently binds checkout launches through the active-PR resolver and records
inferred subjects as `Declared`. A landed branch without an active PR launches
unbound. The approved model allows binding to landed/done Tasks; LOO-298 must
remove the read-time launch-resolver dependency and preserve old attribution
through the one-time migration rather than mistaking S5's limit for policy.

Staging gotcha: `install.py local --skip cargo` bundled a stale `lf`, and the
store gate keys on the registered installation path, not the bytes, so a demo
app must route through the installed `lf` (`LoopflowDevControl.json` →
`lf_path`) or be promoted.

## Continuation and recovery lessons (curated 2026-09-25)

Curated from the retired [continuation record](https://github.com/loopflowstudio/loopflow/blob/1a691ac6a222b95c46859c9c06d162d6442950a4/.lf/directions/task-continuation.md).
Infrastructure owns these execution/recovery lessons by its repository mandate;
this curation does not assign a Task, change a chapter or establish shipment.
The linked record preserves exact historical proof counts and follow-up scope.

- Ordinary and Task execution share captured Flow navigation. Task transactions
  and ordinary file locks remain distinct persistence owners. Advance/Iterate
  choose navigation; completing an Ask returns evidence for reassessment and
  never chooses a verdict. Keyed retries retain the answer. Iteration counts
  describe progress without imposing a pass budget.
- Capture every XOR alternative/router before execution. Recover saved nested
  and human boundaries after their sources disappear; never replace uncaptured
  history with today's catalog. Captured occurrences own human/decision policy.
  Restart can reuse versions and worker generations, so invocation identity must
  fence failure handling and the next claim against late replacement-worker errors.
- A saved candidate is not a completed Run. Failed/interrupted Runs lose decision
  authority. Check the original Run's completion before reclaim replaces its
  binding; consume under the original fenced claim. Recovery fixtures need real
  completion receipts. Cursor settlement alone cannot prove external effects
  occurred exactly once. SQL cursor projection removal needs forward migration;
  captured human Skill tokens still own synchronous prompt preparation.
- Serialize every Ask writer, including reopen reset, under the launch lock.
  Read exact saved feedback during atomic completion, persist completion before
  teardown, and retain published native identity. An unpublished failed launch
  is recoverable only with readable manifest, no published identity and no owned
  live client. The recorded waiver for obsolete human navigation does not waive
  preservation of other execution evidence.
- Prove the driver as well as the reducer: multiple finite provider turns,
  review direction carried into another pass, and recovery without rerunning
  review. Interactive stops and finished Flows never imply Task completion.
  Native handoff must use an executable reading the same Home. Exercise its
  advertised CLI command with a different `lf` first on PATH and overrides absent.
- The live demonstration required manual Flow/Run binding and native provider
  Home corrections. It therefore did not prove automatic propagation. Removing
  stored Codex OAuth injection fixed the observed native-login rejection, but
  forwarded-account leases and ambient Home/auth consistency remain separate
  questions; never generalize that result into credential copying.
- One terminal Wave journal event closes the attempt and restores unfinished
  input claims. Independent chat replies do not claim a governance turn;
  `Inner.open` governs late output. Preserve late-delta and next-turn proofs
  when reducing redundant state. One captured `wave/operate` attempt runs per
  wake; an ordinary catalog Flow can still have callers after its resident is gone.
- Cutover dispositions refer to original sequence/invocation IDs. Only captured
  idle default roots without work retire automatically. Custom, queued, nested
  and uncaptured work retains bytes/order as unresolved evidence. Recovery
  inspection/cancellation does not recompile or launch history. Unknown active
  attempts require termination evidence before cancellation; an absent listener
  does not prove provider death. Arbitrary queue transfer remains unproven.
- Ask fixtures must pin their test executable: mocked launchers still reach
  executable resolution during Session listing. PR #1283 failed in CI when an
  installed development CLI had masked this locally. TESTING.md owns the
  clean-PATH proof. Keep system tools such as `ps` available while excluding `lf`.
- The recorded gate passed 1,988 isolated Rust tests (six skipped), 267 Swift
  tests, eight fixture renders, 229 Python tests, 78 website checks (three
  skipped), architecture, formatting, Clippy and the Xcode fallback build.
  These are prior results with simulated provider effects, not new cleanup
  verification. The default gate receipt `20260925T221844Z-96933-933aee17` failed
  because materialization pinned `LF_CONTROL_HOME` while a fixture isolated only
  `LF_HOME`; the separate Rust pass removed that pin. No green default-gate or
  configured live operation/chat/interruption/failure-recovery claim follows.

Continuation remains scoped to [LOO-295](https://linear.app/loopflow/issue/LOO-295);
broader restoration belongs to [LOO-296](https://linear.app/loopflow/issue/LOO-296).
The historical Wave deletion directive already had implementation; do not
launch a duplicate from that older note. Follow-up Task ownership and installed
acceptance remain unresolved. Historical commit links identify locally recorded
evidence; remote availability was not checked during curation.


## Delivery and chapter implementation lessons (2026-09-25)

Curated from the retired `.lf/prs-and-tasks.md` and chapter direction note.
Current mechanics stay beside their code; these are constraints learned from
implementation, not another chapter plan.

- Preserve authored PR title and opening while adding one managed Task block.
  Keep title/body together through copy resolution. A same-head publication must
  display the persisted merge request, read under the mutation lock; publication
  alone does not request settlement. Landing reads copy before clearing scratch;
  publication consumes only `.pr-copy-ref`, `pr-title.txt`, and `pr-body.md` on
  its own path. Independent review evidence survives publication; gate no longer
  requires a duplicate review report. Release re-arming retains
  remote copy. Prove landing and release consumers when changing that shared path.
- Chapter rotation owns one deterministic boundary. Preserve started Task
  identity, expire untouched backlog and freeze predecessor metric targets,
  readings and evaluation time. Activated retries consume frozen evidence; a
  missing instrument must not prevent a later chapter dropping its target.
- Enumerate fresh provider membership before cutover and recover omitted current
  Projects/Tasks by stable identity. An unavailable read cannot be replaced by a
  cached plan. After activation, retries retain the frozen boundary; external
  reassignment remains unresolved, never authority to reclaim work. Settle a
  lost cancellation response against provider state, not portfolio absence.
- Task claims/retirement share SQLite authority; first execution survives Flow
  resets as Task history. Generic Run history is Home-local. Status and roadmap
  share the Task join, including stranded work; do not rebuild an operator tree
  or duplicate routing fields to flatten it again.
- Repo proof commands now live in TESTING.md instead of standalone local skills.
  Installed-updater compatibility lives in docs/lf.md. The env setup helper is
  `scripts/env-setup.sh`; `.lf/` holds executable configuration and chapter history,
  not a catch-all for maintainer notes. Historical chapter records are retained.

### Draft PR readiness (branch evidence, 2026-09-28)

Jack requested draft-by-default `lf pr open`, preserving an already-ready PR.
Publish/submit/arm/land promote drafts; opening the browser has no readiness
effect. CLI and headless Flow operations share the existing create/update path.
Promotion owns its local readiness update after GitHub succeeds, so a failed
promotion cannot advance Task state or publish ready copy over a draft.

The [preserved realign design](https://github.com/loopflowstudio/loopflow/blob/60ee8daff36b2c97ad5a6f30e1f5daa98a40bca4/scratch/realign.md)
records stateful local GitHub/browser proofs for new drafts, repeated opens,
ready-PR opens, promotion failure and retry, plus focused Task-copy and delivery
checks. The final 27 selected builtin/export/draft tests and all-target Clippy
passed. These are recorded branch checks, not fresh checks from this curation,
real PR mutations, installed acceptance, or proof of Task completion. Historical
links identify local commits; remote availability was not checked.

## PR landing recovery (branch evidence, 2026-09-25)

[PR #1287](https://github.com/loopflowstudio/loopflow/pull/1287) follows Jack's
direction: remove landing blockers rather than add receipt systems.

- **Repair needs ordinary delivery authority.** Etude #187's repair stopped at
  `lf rebase` with `Operation not permitted`: landing's Worktree-only override
  defeated its unattended launch request. More rebases cannot fix that boundary.
  The first denied path was not logged; shared Git metadata was the provider's
  explanation, not a measured filesystem failure.
- **Observe outcomes rather than require activity.** The same SHA can become
  pending, passing, or merged. Neither a new commit nor an unused incident claim
  proves progress. Incidents preserve history; one supervisor owns repair.
  Preserve authoritative merge observations instead of requiring another read.
- **Process success is not repair success.** Use the approved `published` or
  `blocked` result in the existing final answer. Surface the exact human action
  after reconciling GitHub; do not relaunch an impossible repair indefinitely.
- **Queue membership needs explicit cancellation.** GitHub CLI 2.101.0 returns
  success from `pr merge --disable-auto` for a queued PR without removing it.
  Distinguish pending auto-merge from queue membership and use
  `dequeuePullRequest` before publishing a replacement head. The regression's
  remote rejects pushes while queued; a mock command exit cannot prove removal.
- **Fence running effects, not only database writes.** Canceling an async waiter
  does not cancel its blocking repair. A replacement must wait for that operation
  to finish. Active joins must retain the checkout whose supervisor lock is held.
- **Local proof has a boundary.** Simulated provider/GitHub tests cover same-head
  recovery, explicit blockers, and canceled-watcher takeover. Live provider
  permissions, hosted recovery/interruption, and orphan-provider cleanup remain
  unproven. Green-but-unmergeable PRs and the original Etude `-c` attribution
  discrepancy remain unresolved; do not promote these tests into those claims.

Current mechanics belong in [delivery documentation](../../docs/architecture/delivery.md).

### Landing fixtures and deleted remote branches (2026-09-25)

- Fresh landing fixtures use `open_ephemeral_store` or
  `SqliteStore::open_ephemeral`. The constructor owns canonical migrations and
  the draft tail without ambient installation authority. Probing for a table
  and replaying its original migration misses later schema changes. Keep the
  forward migration and populated historical preservation tests for installed
  databases; see [TESTING.md](../../TESTING.md).
- Reproduce remote deletion directly in the bare remote so the checkout keeps
  its stale tracking ref. Deleting through the checkout's own push updates
  tracking and misses the failure. An explicit empty lease permits recreation
  while rejecting a branch created after the absence observation. Existing
  branches retain their tracking lease; fetching to bypass rejection could
  authorize overwriting unseen work.
- Fixture and rebase proofs passed: 17 landing unit tests, 22 rebase tests,
  11 CLI safety scenarios, and the authoritative-merge watcher test. Git
  transport uses local bare remotes; provider/GitHub responses are simulated.
  [PR #1289](https://github.com/loopflowstudio/loopflow/pull/1289) also recreated
  the deleted branch on GitHub using `scripts/dev-lf`; the installed 0.12.21
  command still reproduced the stale-lease failure. That publication proves
  branch recovery, not the other live recovery gaps recorded above.
- The installed `update-wave` skill explicitly sent unnamed-Wave learnings to
  `.lf/`, even after source guidance had changed. Resolve the owning Wave from
  repository objectives and refresh exported skills after changing their source.
  A repository with zero Waves starts with one repo-wide Wave; missing launch
  attribution is not a reason to create another memory location.

## Chapter boundaries and preservation (2026-09-23)

The [accepted chapter](../../.lf/chapters/20260923T000959Z-502f011b/start.md)
keeps Infrastructure responsible for execution mechanics, auth, placement,
recovery, releases, and repository-wide simplification. Product judges external
usefulness; Intelligence owns the non-authoritative evidence. A green scheduling
receipt does not prove publication, and a passing architecture inventory proves
only its declared coverage. Earlier release cadence and backlog notes below do
not override the accepted chapter or current Linear directives.

- **Separate authority from observation.** The summer moved from a remote
  daemon API to complete `lf` execution on each owning Home. Stable Work,
  provider-native continuity, direct child control, and current OS liveness have
  different owners. Run parentage explains causality; it cannot manufacture
  signal authority. The historical reconstruction is a synthesis, not proof of
  one simultaneous fleet deployment.
- **Reduce readers without losing evidence.** CLI and Wave status now share the
  same Work-filtered reader, including its tests. Recent history is bounded;
  exact child discovery is uncapped; activity includes Runs completing inside
  the window even when they started earlier. These answer different questions.
- **Historical archives are outside live architecture discovery.** The checker
  excludes `.lf/chapters/`, as it excludes generated website docs. Fourteen
  retired-term quotations initially failed the check. Preserve those exact
  observations rather than sanitizing history or adding word-specific exceptions.
  The regression still rejects retired vocabulary in active `.lf/skills/` files.
- **PM renames preserve identity but not every historical label.** Chapter
  application found new slugs failing in `lf project status` while stable UUIDs
  resolved the same Work and captured old plan. Use the archived
  `application/project-references.json` identities. Current PM and historical
  execution plans are different facts; never restart a controller or directly
  rewrite storage just to repaint history.
- **Read back shared PM writes from the provider.** During memory curation,
  concurrent branch updates replaced LOO-287's description after a successful
  write. Refresh, merge both evidence sections, and verify the resulting text;
  a successful command or stale local snapshot cannot prove the retained notes.
- **Consolidation does not rebind Task ownership.** LOO-278's files were integrated
  with `lf rebase --manual` into review-chapter, but its registered checkout and
  PR chain still name chapter-planning. Installed `lf task` has no supported
  reassignment; `prepare --name` rejects a different existing workspace and
  `lf work relocate` supports Waves only. Preserve the original tree and exact
  history; a supported adoption path must prove preservation before a new writer
  starts. This is an observed placement gap, not authority to edit the registry.
- **Current PM and historical Work counts differ.** Deduplicate by stable Work
  id before dispositions: the chapter observed 175 rows but 174 unique Works.
  Applied retirement preserved artifacts and history; a PM-complete retired
  issue does not establish shipment or a won KR. Replaying the archive verifier
  proves retained receipts, not fresh PM state or publication.

## Task execution authority (branch evidence, 2026-09-23)

**Owner:** LOO-286, Architecture Minimalism, tracks `jack-heart/wave-agents` in
its existing checkout. The refreshed Linear snapshot explicitly names that
branch; this resolves the scratch design's previously unknown placement. Keep
its Work/PR association with the existing owner; a tracking issue is no reason
to prepare another checkout or launch another writer. LOO-287 owns later
repository-wide reduction. Neither Task is complete from this memory update.

**Chosen contract:** Wave, Project, and Task remain durable Work. Wave and
Project judgments are finite attributed Runs. Task owns implementation pursuit,
its worktree, serial PRs, delivery facts, and current immutable Flow. Helper
attribution grants no cursor or process-control capability; independently
permitted Git/PR/PM operations retain their own authority. Optional Wave chat/API
hosting must not become a prerequisite for Task or Project progress.

At checkpoint `11fa65881`, Project execution and Task first/loop/finally policy
are removed. `task_flow_positions` holds the saved invocation, cursor, worker
claim, review Session evidence, and unsafe blocker. The executor advances that
position directly; Task's synthetic Wave Playhead/body/event adapter is gone.
A worker currently settles one boundary and launches another worker for the
next autonomous boundary. This is narrower than the design's one worker running
to a semantic stop and is not proof of the proposed `TaskExecutionState` API.
Finishing the invocation deletes its row without selecting a successor Flow.
A later launch selects explicit Flow, else the current Project recommendation,
else `task-design`. The Task-level location of the recommendation and automatic
post-completion trigger policy remain unresolved.

### Preserve these boundaries

- Compile expanded definitions at invocation creation. Resume autonomous and
  review Skills from saved content, even when source changes or disappears.
  Catalog validation belongs to new selection, not recovery of a saved Flow.
- Invocation identity must participate in claims and review Session tokens.
  Version and worker generation fence distinct races; restart may reuse their
  numbers, so neither alone identifies the invocation. Validate the launch
  capability before preparation side effects. Never reconstruct a newer review
  position for a late child.
- Worker settlement belongs to Task transactions that atomically fence the
  cursor and write timestamps/events without overwriting newer Task facts.
  Position-only SQLite settlement APIs were test-only alternate writers and
  were deleted. Tests now assert rejected claims leave both domain evidence and
  position unchanged, and accepted evidence appears once.
- Interruption retains the cursor; nonfinal completion advances once; final
  completion keeps the last valid cursor until the transaction removes the row.
  Task completion must never inherit Wave root wraparound. Approve and
  Iterate compare exact saved evidence and clear Session binding on transition.
- Ordinary provider errors remain Run evidence and release the exact claim.
  Unsafe persisted state uses `TaskFlowBlocker`; a missing executable definition
  requires explicit restart and cannot be unlocked by ordinary retry.
- Process death requires existing exact process evidence. Silence, age, tmux
  presence, provider history, and app visibility cannot confer ownership.
  The PR landing watcher is a narrow precedent, not a reason to add another
  general liveness ledger or copy its heartbeat policy into Task takeover.
- Wave's Playhead still serves queued continuations and journal replay. Keep
  historical event decoding after deleting writers. A complete unreadable
  JSONL record must fail without truncating it or later conversation. Historical
  execution needs explicit disposition before restart, including queued work.
- Swift's Project Flow mirror now matches optional `recommended`; migrate Rust,
  Swift, and current JSON fixtures together. Historical migration fixtures stay
  historical. The optional PM Flow envelope distinguishes unknown cached data
  from an explicitly empty recommendation before remote content rewrites.
  Provider labels and Project iteration still have consumers.

### Project reduction (checkpoint `b302db9b5`)

Project now persists planning facts, ownership, completed-operation count,
abandonment intent, and timestamps. The unused `last_state_fingerprint` and
no-op validation API are removed across model, SQLite, and callers.
`project run` composes the existing prepare and launch operations. Independent planning
fact updates still cannot overwrite completed-operation progress. Keep Task
worker claims, Wave queues, and invocation progress: they fence executors or
preserve continuations, unlike the unused fingerprint.

Installed development builds record draft migration checksums too. The forward
`drop_project_fingerprint` draft follows `work_domain_state`; rewriting the
earlier draft would invalidate installed history. Preserve historical fixtures
and prove remaining facts against a populated prior schema. This rule also
lives in TESTING.md with the migration proof commands.

The compression receipt records passing planning-update, populated migration,
sibling-observation, and CLI repository-matrix tests plus Clippy, formatting,
migration, and architecture checks. The matrix uses a missing worker executable
and proves reservation behavior only. It does not supply a real finite Project
Run or settle the broader cutover obligations below; no full CI pass is claimed.

### Evidence limits and remaining obligations

LOO-286 retains the configured CLI/Home/app proof: a real finite Project Run;
concurrent Task starts converging on one owned worker; actual provider death and
same-cursor replacement; late-result rejection; helpers unable to settle; and
exact session continuation across app close/reopen with the optional service
stopped. Repeat Flow selection after changing planning input and prove the old
worker recorded no successor intent. Capture receipts, Run ids, invocation,
version/generation, Session token, process evidence, and truthful failure/wait
states. A seeded store, killed `sleep`, account-preflight error, parser/help
check, or UI fixture proves only its own boundary.

The later slice review's approval does not settle all retained counterexamples.
Source inspection during this curation still finds `work_domain_state` dropping
Task controller progress; `task_worker_claim` copies only existing position rows.
Its test explicitly expects no row for controller-only progress and retains the
old review row for a newer controller cursor. Historical Project positions are
also discarded. Preserve/dispose that evidence explicitly before claiming
lossless cutover; the green migration test does not establish that contract.
Wave startup now stops at historical definitions and exposes explicit restart,
which repairs the earlier silent reset at the source level.

Recorded focused cursor/store/review/DTO checks and static analysis passed in the
compression checkpoints. The earlier full library run had two environment-shared
failures that passed only in isolation; no subsequent full-green run, configured
provider recovery, or desktop proof is recorded. This curation ran no behavioral
suite and does not upgrade that evidence. Production line reduction and bounded
architecture checks are supporting evidence, never acceptance on their own.

### History that constrains the next change

Two one-way deletions removed competing execution authority: `a7044e2b5`
(2026-07-18) removed Session/body leases; `5f7f66833` (2026-08-25) removed the
Invocation/Epoch/Basis stack and the four-day-old `run_liveness` reconciler
introduced by `521ae7d3f`. Durable Work, attributed Runs, Steers, exact review
boundaries, and process evidence survived. `fa0186c4d` (2026-08-28) separated
Work from optional controllers but left launch/recovery dependencies. Prompt
fallback was added (`d76118b7b`), deleted (`309575f8e`), and restored
(`eecde0b2e`) because the ordinary path still failed. Remove that failure in the
operation; deleting recovery prose alone has already failed once.

The app repeatedly replaced control rooms and Ask/attention models; shared
records survived. Keep UI as projection and trigger. Preserve last-good evidence
and missingness, use real Runs and review Sessions, and never invent a synthetic
controller Session or app scheduler. Historical generic `advance_work` proposals
in the research are superseded by Task-only execution; they are not a mandate
to restore Wave/Project cursor symmetry.

The operational incident behind removing `run_liveness` remains unknown; commit
messages establish deletion, not its complete cause. Automatic idle-Task
triggers, finite default wait semantics, worker lifetime, composite Flow source
independence, remaining Wave governance, and installed/runtime projection
consistency must be reconciled with the full design in LOO-286/LOO-287. Do not
split every historical remainder into a new Task or silently widen their KRs.
Research runs twice exhausted their turns without artifacts: write evidence
incrementally and preserve it outside the provider transcript.

## Observation must not manufacture idle work (2026-09-23)

The `task-viewer` repair (`7a41fc0b2`, `d7e221fda`, `aabc03966`) followed two
duplicate helper implementations of LOO-293. Separate readers had reconstructed
identity and liveness from display conventions, and a supervisor treated missing
observations as absence. This branch repairs those readers; it neither adds a
launch lock nor establishes a deployed execution cutover.

- **One catalog resolves Work for Runs, Usage, and Activity.** Read stable
  identity/ancestry columns through a read-only query, not full Project records:
  the latter failed on the installed store's missing `iteration` column. Keep
  the minimal-schema regression. Public issue identifiers, internal Work IDs,
  and external IDs select the same Work. Resolve an unambiguous most-specific
  Work before checking historical ancestor labels; use ancestry to disambiguate
  shared names. Current user filters still apply to today's hierarchy. Preserve
  original manifests and record full ancestry on future bound Runs.
  The catalog is an ephemeral index, not a second durable owner; its selector
  aliases preserve historical Run discovery and are not obsolete compatibility.
- **Receipt ownership survives executable renaming.** Pinned `lf-<hash>` workers
  are legitimate. Visibility and prune share PID/start-identity matching, without
  a basename precondition. Retain reused-PID rejection and keep that fixture's
  PID distinct from the absent OpenCode owner. A completed native launcher says
  nothing definitive about the remaining client or unresolved Session.
- **Sample execution and lifecycle once per Task detail.** TaskExecutionSnapshot
  projects the existing FlowPosition/claim/process evidence for status,
  conditions, actions, and roadmap. Current non-idle evidence wins over dirty
  progress and next-launch configuration failure. An unresolved review boundary
  still yields a waiting condition on terminal Work. Keep lifecycle, worker
  evidence, and UI condition distinct; remove parallel derived Session booleans.
  Run attribution does not acquire advancement or process-control authority.
- **Source proof and promotion are separate.** The incident read found all
  three Runs via public/internal selectors and Usage; source `ps` saw 20 live
  nodes versus installed 0.12.19's two, and dry-run prune excluded all ten live
  Exec PIDs. Nothing was reaped or migrated. Current-schema status has isolated
  test evidence only. Installed projection/cutover proof remains in LOO-286's
  existing obligations; this does not settle it or LOO-293.
  The 2026-09-24 read repeated the three-Run result and excluded all six live
  Execs from dry-run prune (12 live nodes at that sample). Source builds default
  to a development Home: an empty result there says nothing about installed
  history. Use the owning executable/database pair for installed incident reads;
  a branch executable may redirect an explicit installed directory to private data
  under LOO-321's isolation contract above. For isolated tests, clear
  `LF_CONTROL_HOME`, `LF_CONTROL_DB_PATH`, `LF_HOME`, and `LF_DB_PATH`.
  Neither sample proves the older installed Flow schema works.
- **Isolate launch tests from ambient authority.** A research fixture inherited
  the live control database, and a Session fixture failed to publish its fake
  client during the initial broad suite. Isolated reruns passed after clearing
  execution authority. Fixture cleanup may stop
  only its identified fake provider child. A passing DTO round-trip alone cannot
  prove execution precedence.

## Prompt reduction boundary (2026-09-24)

The `task-viewer` branch removes the unused prompt-direction feature end to end
and collapses unchecked gather/render wrappers to `PromptComponents` and `String`.
The context-specific contract is in [Intelligence memory](../intelligence/MEMORY.md#prompt-assembly-reduction-branch-evidence-2026-09-24).
Keep reductions tied to actual ownership: `WorkCatalog` preserves historical
identity lookup, `TaskExecutionSnapshot` distinguishes current execution from
durable Work disposition, and `PreparedLaunchPrompt` carries consumed evidence.
None is interchangeable with the deleted wrappers. LOO-287 still owns the broader
architecture pass and its real weekly observations; local deletion and passing
checks do not complete that Task or establish its KR.

## Installation and command scope (branch evidence, 2026-09-24)

The `global-cmds` branch separates machine installation from checkout updates.
Infrastructure's existing release/recovery mandate owns these learnings;
this curation does not bind the branch to a Task or change Wave identity.
LOO-292 retains installation/rebase acceptance; LOO-287 retains command-scope
reduction follow-ups. Both remain open. Their updated notes supersede the older
combined laptop/package/main refresh requirements, without discarding the
reported main-rebase failure or its preservation obligations.

### Keep the ownership boundaries

- `lf install` selects the latest published release and verifies its pinned
  shell installer. The shell downloads artifacts; the existing candidate
  promotion transaction alone creates/advances the store and activates bytes.
  `lf rebase` owns checkout refresh/integration and keeps its journal evidence.
  Installation must not run Git, Homebrew, uv, Python or source-tree maintenance.
- First install has no prior selection or previous published fallback. Before
  candidate handoff it can cancel to uninstalled; afterward the pinned candidate
  must recover and settle. Ordinary commands cannot use an uncommitted first
  selection. Read-only install preflight must reach its own authority checks
  before ordinary startup authorization, or first-install recovery deadlocks
  on the absent prior selection. Existing-store adoption still proves its
  store/artifact authority.
- Matching CLI/daemon versions alone do not mean current: exact-store preflight
  must succeed without migration, and macOS must have the complete matching app.
  Bound subprocess inspection so broken installed binaries cannot hang updates.
- Older CLIs transition through the external installer. A retained Python
  `refresh` alias delegating to PATH `lf install` recurses through the old CLI's
  source updater. The alias and dependent wrapper are deleted; do not restore
  them as a compatibility bridge.
- Optional discovery returns a real checkout or absence, preserving genuine
  Git failures. `CanonicalRepo::current` cannot accept an ordinary folder as
  repository scope: doing so produces plausible empty Wave/Session listings.
  Stored-locator discovery and folder prompt `working_directory` have different
  contracts. Provider `RepoId` still owns origin identity and Git URL rewriting.
- Machine commands bypass repository runtime capture; catalog/flow inspection
  must not create journal attribution. Keep explicit target/default-route
  resolution independent of irrelevant caller Git. Do not replace missing repo
  identity on a route mutation with a default-route write.
- Scheduled installation retains its existing launchd label/log destination
  and custom `LF_INSTALL_DIR`, but no source WorkingDirectory or Python. The
  accepted cadence is login plus weekly by default (Monday 09:00), with daily
  09:00, hourly, or five-minute clock intervals available explicitly through
  `lf install schedule [weekly|daily|hourly|5min]`. The clarified requirement is that
  cadence is positional, without `--every`. Calendar scheduling should coalesce
  sleeping intervals. The concurrent scheduling receipt records a public-CLI
  cadence matrix and static passes with simulated launchctl; match final proof
  to the positional-argument bytes. It does not prove actual login/wake events.

### Evidence and demo traps

Review through `ac0b51aad` records 67 focused tests and static checks, plus three
earlier checkout tests. The disposable Ubuntu 24.04 candidate demo passed fresh
installation without Git, repeat without asset downloads, missing-daemon repair,
dirty-checkout/ref and migration-ledger preservation, failed-activation recovery,
and external transition from authentic checksum-verified 0.12.18 binaries.
That older account had no populated historical Home. The candidate reported
0.12.19 with drafts materialized only in a disposable source snapshot; local
HTTPS release transport is not a public release. Captured logs and full audit
remain in branch history at `1f2d2c051:scratch/` after scratch is cleared.

The real published 0.12.20 demo failed clean-home promotion after preflight
accepted the absent store. Preserve that observation; candidate success cannot
rewrite it. Public-channel acceptance needs a released fix and repeated
fresh-install/repair proof. Real macOS app installation and populated historical
Home migration are still unproven. No full CI or current-tree pass is implied.

- Machine installation resolves the OS account home, ignoring `HOME` overrides.
  `HOME`/`LF_HOME` alone cannot isolate promotion. Use a disposable OS account or
  container with no real installation mounted; remove obsolete installer tests
  whose mocks now permit the real downloader. PATH mocks for Homebrew or uv do
  not intercept HTTP downloads or promotion. Include failed first activation and
  pinned-candidate recovery: fresh-install success alone misses dependencies on
  prior startup selection. See `TESTING.md` and `tests/e2e/install_bootstrap.py`.
- Debian bookworm could not run the downloaded release's GLIBC_2.38/2.39
  requirements; Ubuntu 24.04 reached promotion. Distinguish platform failure
  from repository discovery failure.
- Wait for artifact copies before packaging/serving, then compare archive-member
  SHA-256 with source/runtime bytes. An asynchronous copy race produced a
  different CLI and a segfault; the daemon matched. TLS fixtures also need a
  separate server leaf signed by their test CA. These were fixture failures.
- Removing Git from PATH must retain unrelated runtime prerequisites such as
  `ps`. A missing prerequisite is not evidence that the command needs Git.

### Reduction and unresolved scope

The shell bootstrap and installed release selector share one promotion owner.
Artifact bytes, selected install/store, retained published fallback, and durable
switch progress remain distinct. Advancement may finish while phase still says
`Advancing`; phase alone loses recovery evidence. Persisted duplicate receipt
facts need a coherent format change and interrupted existing-install proof,
not isolated field deletion. No such redesign was selected in compression.

LOO-287 retains target-first Work/PM/cron/chat/bound-invocation resolution and
ordinary-folder execution through provider completion without an implicit Git
checkpoint. Recheck downstream credentials/config and explicit worktree flags;
no capability registry, skill-name allowlist, fake repo, or swallowed Git errors.
The scope audit is source evidence, not proof that every command ran. Starting
a new repository remains an unselected design question: separate Git creation,
local configuration, optional provider connection and later PM setup. Today's
`init` connects an existing repository to the distributed system.

## Shipped

- **Install syncs skills** — installation runs `lf sync-skills --yes` after installing `lf`, so `~/.claude/skills` and `~/.agents/skills` track the freshly installed binary. Sync failure warns but never fails the install; the binary is already in place. The former combined repo-refresh path is superseded by the installation contract above.
- **Deterministic rebase & placement** (rebase-efficiency parent) — `lf rebase` classifies the branch via merge-base diff *before* touching git and picks reset / direct-rebase / rebase-onto-parent / skip-parent-onto-main / noop; only genuinely conflicting authored work escalates to the rebase agent. Disposable branches (no unique commits, generated/checkpoint-only, scratch-only) reset to base instead of burning a long rebase. `scratch/` survives via directory copy to `.lf/tmp/scratch-stash/<branch>-<ts>/`. `--plan` prints the deterministic decision without mutating git. Ops telemetry → ignored `.lf/tmp/metrics/ops.jsonl` (strategy/class/counts, no diffs or secrets). Classifier uses merge-base diffing so upstream-only drift isn't counted as local authored work. E2E: `tests/e2e/test_rebase_efficiency.sh`. This directly attacks the "avoidable long rebase" sharp edge in the daily loop.
- **Worktree redesign, stages 1–3a** (PR #818) — fixes the #802 fallout: runaway nesting (`loopflow.jack-heart.bugs.20260705_1627.goals`), wave identity that stopped resolving, and land rotation renaming the worktree out from under a running agent.
  - **Placement (stage 1):** `lf wt create <name>` creates a low-level sibling worktree; `--plan` previews placement without writing. Task and Project Work own their higher-level worktrees. The retired `--fork`/`--main`/`--stack` and intermediate `--sibling`/`--child` flags are not part of the current CLI.
  - **Identity (stage 2):** `WaveId` (`engine/identity.rs`) — one identity, **two decoupled projections** that are not string-derivable from each other: `dir_component()` = flat `{chain}[.ts]` (author-free, the local worktree dir `{repo}.{dir_component}`) and `branch()` = `{user}/{chain}[.ts]` (author-scoped `/`, glob-able `jack/**`, **remote only** — a `/` can't go in a worktree dir). The dir↔branch link lives on the `Run` record, not string surgery. Wave name = chain segment 0 (keys `wave/<name>/`, chat, pm). Waves/subwaves are stamp-free; workers carry one trailing `.ts` minted at dispatch — the stamp's *presence is the worker marker*. `parse(raw, fallback_user)` is the single input funnel (Postel: liberal in, strict out). `BranchNameConfig`/`branch_names` schema, `format_branch_name`/`generate_word_pair` rotation naming — all retired. Data model doc: `rust/loopflow/src/wave/DATAMODEL.md`.
  - **Land (stage 3a):** land rotation killed — `rotate_worktree`/`RotationResult` removed. A land never renames the live worktree; the **wave home is permanent** (`<repo>.<wave>` on a stamp-free branch). Workers self-prune once merged; direct commits from the wave home stay possible but discouraged (soft LOOPFLOW.md guidance, not a hard block).
  - **Rejected delimiters:** `@` (legal in refs but outside GitHub's safe set, shadows `@{upstream}`, breaks pip/CI URL parsing) and `:` (illegal in refs, Finder renders it `/`). Research: every mature stacking tool (Graphite/Sapling/ghstack/gh) keeps lineage in metadata, not the name — so the chain in a name is a *hint*, never parsed for truth (loopflow already has the DAG in `Run.parent_run_id`/`stack_group_id`/`stack_position`).
- **`lf pm` speaks wave/project/task** (PR #852) — `status`, `show --project <slug>`, `task create/update/done/move`, `rename`, `sync --plan`. The Linear `teamId` `String!`-vs-`ID!` bug is fixed: creating and closing tasks from the CLI works. `update` survives as a compat alias; the documented path is `task …`.
- **Native Linear hierarchy for PM** (jack-heart/infra) — Wave→Linear Initiative, Project→Linear Project, Task→Issue, replacing the wave-project-plus-`project:<slug>`-label model. `GOAL.md` frontmatter now carries `pm.linear_initiative`; `lf pm init` creates the Initiative, migrates legacy labeled issues into native Linear Projects, and rewrites `pm.linear_project`→`pm.linear_initiative`. Project definition/KRs round-trip through Linear Project `content`. The live Waves are migrated; Linear is the planning truth.
- **Linear OAuth token pre-emption** — Linear PKCE access tokens expire in 24 h. Loopflow now persists the non-secret OAuth client ID beside the token (migration `060_provider_token_oauth_client_id`) and refreshes ~20 min before expiry, both on PM access and in the background `token_refresh` trigger. PKCE refresh needs no client secret. A rotated refresh token is persisted; an omitted one preserves the prior token. Proactive-refresh failure while the access token is still valid falls through to the current token and retries later; an expired legacy row with no OAuth config fails safe with a sanitized one-time reconnect command. Directly serves developer-efficiency's "credential expiries pre-empt" KR.
- **`lf pm show` renders an aligned table** — one task per physical line under stable headers, columns measured from visible content (shares the `lf wt list` padding primitive), open tasks before done while preserving Linear rank within status, full task IDs kept, `--json` unchanged for machine consumers. Long titles can no longer collide with project/assignee/ID fields.
- **Cron continuity follows durable obligations** (LOO-241) — installed fixed-daily jobs persist their activation time across unchanged syncs, and legacy jobs recover it from the earliest matching scheduled receipt. `lf doctor` judges only each job's latest due interval against an exact scheduled receipt; failed targets still prove the scheduler fired, manual receipts do not, and a miss names the cron, Home, interval, and history command. Raw ledger gap days remain visible history without keeping later telemetry red.

## Gotchas

- **`scripts/test.py --all` cannot green the Loopflow UI suite headlessly** (filed). `xcodebuild` runs 304 app/unit tests to a pass, then `LoopflowUITests-Runner` hangs before establishing its connection and Xcode exits 65. Reproduced with a fresh `derivedDataPath`, so it is not a stale-cache artifact. Treat a `--all` UI failure as unproven, not as a regression, until the runner hang is fixed.
- **Dotted-root vs dotted-ancestry collision — RESOLVED** by the WaveId decoupling: the dir is a flat `.`-chain, the remote branch carries `/`+author, and ancestry is read from the `Run` record, not the string. The old `branch_names.schema` grammar that caused it is gone.
- **Run `cargo test` to completion before trusting a green-looking suite.** A failing lib target makes cargo skip every later target, so lib failures mask bin failures — two `bin/lf.rs` tests naming a deleted command had never run at all.
- **Rust compilation does not validate SQLite column names.** Runtime SQL whose shape depends on a released schema must be shared with a behavior test that prepares and executes it against the materialized migration head. Epoch Work ownership is three exclusive foreign keys (`wave_id`, `project_id`, `task_id`); generic kind/id belongs to explicit routes such as synchronous cross-Work questions, not to Epochs.
- **Source history must reconstruct every applied release frontier** (learned 2026-07-20). One pre-schema-closure local promotion embedded a test-materialized `0.12.4` batch and advanced the shared store while git retained the ten source drafts and omitted the canonical file. Recovery preserved the database, extracted the canonical bytes from the retained immutable binary, matched their checksum to `schema_migrations`, registered the batch, and removed only byte-identical drafts. If a store is ahead by an unknown migration, retain state and old binary bytes; prove the checksum before ratifying history. Since #1123, draft-bearing candidates fail promotion even at an exact frontier, while a schema-complete exact-frontier CLI repair may safely activate with live Runs because it writes no migration.
- **Tests must survive draft migration materialization** (learned 2026-07-21).
  Release-equivalent Rust tests delete ordinal-free drafts and compile the
  generated canonical batch. Test fixtures resolve migration SQL by its draft
  marker through `migration_sql_for_test`; an `include_str!` pointing directly
  at `migrations/drafts/` passes locally and fails the release tree at compile
  time.
- **Ordinary-PR integration tests inherit Task authority inside a worker.** Scrub `LF_RUN_CONTEXT` (plus its lease/invocation companions) when a fixture deliberately represents a non-Task repository. A missing registry while Run context is present is the intended fail-closed behavior, not a commit/push regression.
- **Concurrent editing corrupts a file; concurrent rebasing corrupts history.** Two drivers sharing one worktree shared its `rebase-merge` state dir: conflicts resolved themselves between one command and the next, and `done` advanced 6→22 with no `--continue` from the losing session. Nothing was lost that time. Check for a live agent before working — or rebasing — a wave worktree; the driver that owns the worktree owns its `.git` sequencer.
- **Linear Project UUIDs survive renames; derived slugs do not.** Project content
  lives in Linear and the local SQLite snapshot, with no `projects/*.md` cache.
  Use stable IDs when reconciling current names with captured historical plans.
- **Environment configures a process; it must never decide what the process is.** An earlier runtime chose between booting a listener and being a resident from inherited environment, so a promoted wave could attach to its parent's listener with the parent's token. The current `lf wave` surface keeps that role explicit.
- **Current PM truth and durable Work history have different lifetimes** (learned 2026-07-21). A terminal Project omitted from the current PM snapshot can still own non-terminal historical Task Work. Wave reads must render the current PM hierarchy and classify the stranded Project/Task separately as Wave-owned degraded evidence; they must not fail the whole join, delete history, or synthesize a PM Project. Preserve stable identity when the historical checkout is absent. LOO-305 removes the old local-only `work abandon` recovery command; inspect retained facts with explicit `lf task status <task-id>`. Missing Project evidence alone never authorizes provider deletion.
- **Terminal Task state and current PM routing are authorization boundaries**
  (learned 2026-07-21). An open Linear issue, inherited direction, or sibling
  completion is evidence, never permission to reopen `Done` or `Abandoned`
  Work. Recovery from abandonment requires explicit User authority. When
  Linear moves an issue, historical Task Runs retain their evidence but lose
  automated PR and completion authority; fail closed before side effects and
  preserve the full Work, Run, Steer, and PR history for remediation.
- **Historical continuity currently short-circuits daily telemetry** (observed
  2026-08-23). `telemetry-daily` stops in `doctor` on the same eight 2026-08-04
  through 2026-08-11 gap days before its scorecard runs. LOO-241 owns making
  continuity obligation-aware. Fresh receipts are new evidence for that Task,
  not grounds for duplicate daily Tasks; retry its Work only from a Turn with
  valid Run execution context.
- **Release orchestration and product publication are separate evidence**
  (observed 2026-08-23). A cron receipt proves only the scheduled target's
  terminal state. `lf release status` remained at tag `v0.12.14` with a
  successful hosted workflow and gate-safe notes but no GitHub Release after
  both successful and failed `release-run` receipts. Judge the release KR by
  the product state and keep same-tag recovery singular; LOO-261 owns the known
  clean-host candidate-validation boundary.
- **Incomplete release synchronization still consumes caller edits**
  (reproduced 2026-08-23). The scheduled retry left `main` clean after removing
  two pre-run Infrastructure memory edits. LOO-266 owns preserving the caller
  branch, index, and working bytes across every release exit; do not file a
  second repair Task for later instances of the same failure.

## Model (design settled)

- Self-hosting is the default. The public repo carries containers, deploy scripts, service units, schedules, and docs; secrets live in Doppler or host-local env, never git.
- Nightly verifies release-grade artifacts with no publish or deploy side effects; weekly publishes only after equivalent verification passes in the same run.
- Loopflow carries the primitives; Cadenza mirrors the cadence and shape until a product-specific difference is deliberate and documented.
- Don't extract a generic multi-product deploy platform before a second or third real deployment proves the shape.
- Release owns the automation spine, not release-content substance: each product owns its own changelog and provider-specific agent credentials (beyond pass-through/secret wiring).
- **One writer per worktree is dispatch discipline, not a general lease**
  (decided 2026-07-10). Worktrees are cheap and placement already exists, so a
  second writer belongs in another worktree. The store contributes visibility;
  mutation-specific coordination may still use a narrow local lock, as exact-head
  PR finalization now does.
- **The database is durable control state, not a message bus.** Radio,
  `bus_messages`, `bus_cursors`, channel identity, bylines, and retention are
  deleted. Authored input is a durable Work Steer. Observation delivery must
  preserve input without requiring a resident Project. Automatic dispatch policy
  remains open; a nudge must not invent a second execution authority.
- **Wave chat connection retains one event-driven Home lifecycle** (reconciled
  2026-09-28). LOO-321 removes the explicit start/stop surface; opening chat
  connects through the local Home's `lf`/`lfd` pair without promoting or
  replacing binaries or moving remote placement. Daemon boot
  publishes one attempt-scoped durable `live | failed` receipt and uses a
  private socket only as the wake edge; `lfd` owns listeners and shares each
  listener's `starting | live | failed` transition with concurrent callers.
  Success drains the durable observation outbox before returning. Failure
  compensates only registry state introduced by that attempt, and one failed
  Wave never terminates successful siblings. The Mac app uses the shared
  connection path. Reconciliation polling remains recovery,
  never startup acknowledgement.
- **Controller evidence is not an agent Run** (learned 2026-07-20). When a
  merged PR or another controller fact completes a Task, persist the Task
  domain transition and completion event in one transaction. Never mint a
  synthetic Run to reuse a Run-owned terminal transition. Prove this boundary
  with a zero-agent-boundary fixture and repeated reads that count Runs and
  completion events.
- **Execution authority changed on the Task-worker branch.** The former
  resident session, lifecycle/gate epochs, mutable Flow pins, and parent Turn
  Basis are historical models. Use the Task execution section above for current
  ownership, recorded decisions, failure evidence, and migration constraints.
- **Performance evidence preserves missingness at every boundary** (learned
  2026-07-21). A provider receipt absent, one missing field, and a reported
  zero are distinct facts; the first accepted per-Turn receipt wins and a
  conflicting repeat makes capture partial without rewriting spend. Window a
  scorecard by the owning fact's terminal time, never its parent's start time,
  and publish eligible/measured coverage beside every percentile. An absent
  authority is a named `UNKNOWN`, not permission to infer from observer
  timestamps, trace text, or zero. Budgets judge evidence; they do not change a
  correctness result.

## Planning model (settled, PR #852)

- **Three nouns, distinguished by kind, not size.** Wave = durable operating context (memory, cadence, budget, chat, project selection). Project = one measured bet inside exactly one wave, a definition plus KRs. Task = a concrete change. No project trees, no orphan projects.
- **Where each noun lives.** Wave = `wave/<wave>/` (`GOAL.md` + `MEMORY.md`). Project = a Linear Project under that Wave's Initiative. Task = a Linear Issue under exactly one Project. Local Project and roadmap mirrors are deleted; SQLite is a read model, not a second authoring surface.
- **Native Linear hierarchy (shipped, jack-heart/infra): Wave → Linear *Initiative*, Project → Linear *Project*, Task → Linear *Issue*.** Supersedes the label model below. The wave anchors on `pm.linear_initiative`; `lf pm init` creates the Initiative, migrates each legacy `project:<slug>`-labeled issue into a native Linear Project (moving it via `move_item_to_project`), writes `linear_initiative`, and drops `pm.linear_project` **only** once every legacy task carried exactly one recognized label. A task with zero or >1 recognized labels is left behind, `pm.linear_project` is retained, and the `unmigrated` count is reported so the operator assigns the label and re-runs `pm init`. Project **definition + KRs live in Linear Project `content`** (`## Definition` / `## KRs` checkbox Markdown), the one-line summary in `description`; Loopflow parses them into typed `PmProject { slug, summary, definition, krs: Vec<PmKr{text,holds}> }` rather than leaking the storage convention. `holds` is an explicit `[x]` judgment, not derived evidence. A **duplicate or empty derived slug is a hard drift error** (silently choosing one would weaken exactly-one-wave). Seeding is **restart-safe**: `pm init` writes a transient `pm.linear_seed_pending` marker after creating the Initiative and before seeding Projects, resumes only the missing Projects on re-run, and clears the marker on clean completion. Linear permits a Project in many Initiatives; Loopflow enforces exactly-one-*wave* at its own layer and leaves unrelated associations alone. No local Project cache survives as an alternate source of truth.
- **Superseded — label model (PR #852):** one Linear project per wave, Loopflow projects as `project:<slug>` issue labels. Was the incremental-migration bridge; the native hierarchy above replaces it and reads legacy labels only as migration input.
- **Open (native hierarchy):** standing quality-frontier projects have no natural Linear completion date; the API allows date-less projects, so leaving frontier bets undated is a product convention, not a schema blocker — don't force a target date on them.
- **Vocabulary discipline.** Say "Linear project" for the Linear object, "project" for a Loopflow measured bet. No fourth noun — "space" and "provider container" were considered and rejected as user-facing words.
- **`doctor --planning` diagnoses; it never guesses.** LOO-305 consolidates the former `pm doctor` and `pm sync --plan` entry points here. Ambiguous Task moves remain diagnostic findings for review; diagnosis does not apply them.

## Earlier follow-ups (reselect through the accepted chapter)

- **Reduction leftovers from the `minds` review** (triaged; the `TurnFinished`+`BodyFinished` collapse, the `LoopRun` reuse in `bin/lf.rs`, and the stale `playhead.rs` error hint are applied): factor the shared inbox-interrupt arms and lift the lease-renewal block; merge `interrupt_child`/`interrupt_harness` behind one `begin_interrupt`; finish the endpoint-resolver consolidation; inline `require_loop_flow`. `heartbeat_idle` stays — a real scheduler input, and deleting it to satisfy a lint instinct is reshaping production code around tests in reverse.
- **Live Work/Launches per worktree in `lf status`** — the store already holds
  their cwd. Visibility, not a general lease (see the one-writer rule above):
  typing into an occupied tree should be a choice made with open eyes, not a
  discovery made in history.
- **Concurrent PM reads on status/sync** — `lf pm show` fetches per-project issue lists concurrently, but `pm status` and `pm sync` still read them sequentially. File if sequential reads become a measured bottleneck.
- **Drain current buffer** — keep local `lf`, release scripts, and CI aligned with the latest merged release-infra work.
- **Cadenza release parity** — same nightly/weekly cadence, one-command updater, tests, self-hosted assumptions; document any deliberate divergence.
- **Cron host bootstrap** — bring up the first maintained `lf cron` host (Mac mini default), Doppler configured, with scheduled checks.
- **Release feedback loop** — failed nightly/weekly runs surface as attention items or focused fix PRs, distinguishing verification vs publish vs host vs stale-local drift.
- **Installed-upgrade semantic gate** — preserve saved Task invocations and
  historical stops through migration; validate new selections against the
  candidate catalog without re-resolving active definitions.
- **Project terminal-receipt parity** — make Project failure events and
  Run/Invocation settlement share the atomic receipt boundary now used by
  Tasks, with a fault-injection proof.
- **Replicate intentionally** — apply the skeleton to Manabot/Hootro only when they need it.

- **Deferred: "up/down 5ths"** (Jack, 2026-07-06) — referent unresolved. `lf wt` shipped up/down stack navigation this branch; candidates for the phrase are stack level-jumps ("fifth" = a level), circle-of-fifths name generation instead of random word pairs, or a chord-model transpose. Jack said "keep going" — deferred, not dropped.

The rebase-efficiency follow-ups are resolved by PR #818: config/naming-schema redesign shipped as `WaveId`; `lf wt create` is sibling-only; Task and Project Work own higher-level worktree placement; land rotation and `next`/`advance` are removed.

### How to judge rebase efficiency (dogfood metrics from `.lf/tmp/metrics/ops.jsonl`)

Local-only JSONL, reviewed weekly. Key product metrics: **agent-rebase rate** (% of rebases launching an agent), **avoidable rebase-agent rate** (stale/empty/generated-only branches that still launched one — target 0), median `land`→queued/merged time, post-land repair rate, and command-drift rate (prompt-recommended commands the installed `lf` can't parse). Then flip one default at a time: stack-by-default `wt create`, stale-empty reset before rebase, land/advance split, generated-only reset policy. Synthetic-workload replay harness (50–100 disposable histories, current vs classifier in trace mode) is unbuilt — file if tuning thresholds needs it.

## Direct invocation and large inputs (2026-09-25)

Work selectors give direct skills/flows attribution, context and placement;
`lf task run` owns the managed Task Flow. Direct bound contributions receive
fresh scratch and leave checkpointing to their caller. Bare names prefer skills;
explicit verbs resolve their own kind. Reuse the skill-to-invocation loader,
without one-skill wrappers or name-specific dispatch. Started is written at
explicit interactive/headless CLI dispatch, after capture and before provider
launch. Generic capture stays registry-independent: putting Started there broke
the unavailable-registry regression. Read-only Work resolution and unopened review
preparation never record execution. LOO-298's derivation from Run rows replaces
this write only when that model is implemented.

Recursive scratch exceeded both argv capacity and a provider input limit.
Claude batch input uses text stdin backed by an anonymous file, with system
instructions in the existing context file; captured and streamed output use the
same launch path. Curate scratch instead of silently truncating instructions.
The observed Codex rejected `turn/start` remained waiting; that driver failure
is still unresolved, and Claude's working input path does not establish a fix.
