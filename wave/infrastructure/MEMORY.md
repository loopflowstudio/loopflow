# infrastructure wave memory

Renamed from `systems` in the 2026-07-08 wave/project/task restructure. Owns dependable self-hosting, verified releases, and architecture minimalism. The configured release schedule and accepted proof obligations govern current work; older nightly/weekly notes below are historical.

## Delivery and chapter implementation lessons (2026-09-25)

Curated from the retired `.lf/prs-and-tasks.md` and chapter direction note.
Current mechanics stay beside their code; these are constraints learned from
implementation, not another chapter plan.

- Preserve authored PR title and opening while adding one managed Task block.
  Keep title/body together through copy resolution. A same-head publication must
  display the persisted merge request, read under the mutation lock; publication
  alone does not request settlement. Landing reads copy before clearing scratch;
  publication consumes gate artifacts on its own path. Release re-arming retains
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
- **Fence running effects, not only database writes.** Canceling an async waiter
  does not cancel its blocking repair. A replacement must wait for that operation
  to finish. Active joins must retain the checkout whose supervisor lock is held.
- **Local proof has a boundary.** Simulated provider/GitHub tests cover same-head
  recovery, explicit blockers, and canceled-watcher takeover. Live provider
  permissions, hosted recovery/interruption, and orphan-provider cleanup remain
  unproven. Green-but-unmergeable PRs and the original Etude `-c` attribution
  discrepancy remain unresolved; do not promote these tests into those claims.

Current mechanics belong in [delivery documentation](../../docs/architecture/delivery.md).

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
claim, human Session evidence, and unsafe blocker. The executor advances that
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
  human Skills from saved content, even when source changes or disappears.
  Catalog validation belongs to new selection, not recovery of a saved Flow.
- Invocation identity must participate in claims and human Session tokens.
  Version and worker generation fence distinct races; restart may reuse their
  numbers, so neither alone identifies the invocation. Validate the launch
  capability before preparation side effects. Never reconstruct a newer human
  position for a late child.
- Worker settlement belongs to Task transactions that atomically fence the
  cursor and write timestamps/events without overwriting newer Task facts.
  Position-only SQLite settlement APIs were test-only alternate writers and
  were deleted. Tests now assert rejected claims leave both domain evidence and
  position unchanged, and accepted evidence appears once.
- Interruption retains the cursor; nonfinal completion advances once; final
  completion keeps the last valid cursor until the transaction removes the row.
  Task completion must never inherit Wave root wraparound. Human Approve and
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
exact human continuation across app close/reopen with the optional service
stopped. Repeat Flow selection after changing planning input and prove the old
worker recorded no successor intent. Capture receipts, Run ids, invocation,
version/generation, Session token, process evidence, and truthful failure/wait
states. A seeded store, killed `sleep`, account-preflight error, parser/help
check, or UI fixture proves only its own boundary.

The later slice review's approval does not settle all retained counterexamples.
Source inspection during this curation still finds `work_domain_state` dropping
Task controller progress; `task_worker_claim` copies only existing position rows.
Its test explicitly expects no row for controller-only progress and retains the
old human row for a newer controller cursor. Historical Project positions are
also discarded. Preserve/dispose that evidence explicitly before claiming
lossless cutover; the green migration test does not establish that contract.
Wave startup now stops at historical definitions and exposes explicit restart,
which repairs the earlier silent reset at the source level.

Recorded focused cursor/store/human/DTO checks and static analysis passed in the
compression checkpoints. The earlier full library run had two environment-shared
failures that passed only in isolation; no subsequent full-green run, configured
provider recovery, or desktop proof is recorded. This curation ran no behavioral
suite and does not upgrade that evidence. Production line reduction and bounded
architecture checks are supporting evidence, never acceptance on their own.

### History that constrains the next change

Two one-way deletions removed competing execution authority: `a7044e2b5`
(2026-07-18) removed Session/body leases; `5f7f66833` (2026-08-25) removed the
Invocation/Epoch/Basis stack and the four-day-old `run_liveness` reconciler
introduced by `521ae7d3f`. Durable Work, attributed Runs, Steers, exact human
boundaries, and process evidence survived. `fa0186c4d` (2026-08-28) separated
Work from optional controllers but left launch/recovery dependencies. Prompt
fallback was added (`d76118b7b`), deleted (`309575f8e`), and restored
(`eecde0b2e`) because the ordinary path still failed. Remove that failure in the
operation; deleting recovery prose alone has already failed once.

The app repeatedly replaced control rooms and Ask/attention models; shared
records survived. Keep UI as projection and trigger. Preserve last-good evidence
and missingness, use real Runs and human Sessions, and never invent a synthetic
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
  progress and next-launch configuration failure. An unresolved human boundary
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
  history. Select the intended Home explicitly for read-only incident checks;
  clear `LF_CONTROL_HOME`, `LF_CONTROL_DB_PATH`, `LF_HOME`, and `LF_DB_PATH` for
  isolated tests. Neither sample proves the older installed Flow schema works.
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
  `lf install schedule [weekly|daily|hourly|5min]`. The human clarified that
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
- **Current PM truth and durable Work history have different lifetimes** (learned 2026-07-21). A terminal Project omitted from the current PM snapshot can still own non-terminal historical Task Work. Wave reads must render the current PM hierarchy and classify the stranded Project/Task separately as Wave-owned degraded evidence; they must not fail the whole join, delete history, or synthesize a PM Project. Recovery must use the stable Work id (`lf work abandon task <work-id>`) because higher-level Task commands may inspect a historical worktree that no longer exists.
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
- **Supported Wave startup is one event-driven Home lifecycle** (decided
  2026-07-21). `lf start` opens the selected Home registry and uses its current
  `lf`/`lfd` control pair without promoting or replacing binaries. Daemon boot
  publishes one attempt-scoped durable `live | failed` receipt and uses a
  private socket only as the wake edge; `lfd` owns listeners and shares each
  listener's `starting | live | failed` transition with concurrent callers.
  Success drains the durable observation outbox before returning. Failure
  compensates only registry state introduced by that attempt, and one failed
  Wave never terminates successful siblings. The Mac app uses the same
  `RegistryQuery.start` receipt path. Reconciliation polling remains recovery,
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
  ownership, human decisions, failure evidence, and migration constraints.
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
- **Native Linear hierarchy (shipped, jack-heart/infra): Wave → Linear *Initiative*, Project → Linear *Project*, Task → Linear *Issue*.** Supersedes the label model below. The wave anchors on `pm.linear_initiative`; `lf pm init` creates the Initiative, migrates each legacy `project:<slug>`-labeled issue into a native Linear Project (moving it via `move_item_to_project`), writes `linear_initiative`, and drops `pm.linear_project` **only** once every legacy task carried exactly one recognized label. A task with zero or >1 recognized labels is left behind, `pm.linear_project` is retained, and the `unmigrated` count is reported so a human assigns the label and re-runs `pm init`. Project **definition + KRs live in Linear Project `content`** (`## Definition` / `## KRs` checkbox Markdown), the one-line summary in `description`; Loopflow parses them into typed `PmProject { slug, summary, definition, krs: Vec<PmKr{text,holds}> }` rather than leaking the storage convention. `holds` is a human/loop `[x]` judgment, not derived evidence. A **duplicate or empty derived slug is a hard drift error** (silently choosing one would weaken exactly-one-wave). Seeding is **restart-safe**: `pm init` writes a transient `pm.linear_seed_pending` marker after creating the Initiative and before seeding Projects, resumes only the missing Projects on re-run, and clears the marker on clean completion. Linear permits a Project in many Initiatives; Loopflow enforces exactly-one-*wave* at its own layer and leaves unrelated associations alone. No local Project cache survives as an alternate source of truth.
- **Superseded — label model (PR #852):** one Linear project per wave, Loopflow projects as `project:<slug>` issue labels. Was the incremental-migration bridge; the native hierarchy above replaces it and reads legacy labels only as migration input.
- **Open (native hierarchy):** standing quality-frontier projects have no natural Linear completion date; the API allows date-less projects, so leaving frontier bets undated is a product convention, not a schema blocker — don't force a target date on them.
- **Vocabulary discipline.** Say "Linear project" for the Linear object, "project" for a Loopflow measured bet. No fourth noun — "space" and "provider container" were considered and rejected as user-facing words.
- **`sync --plan` diagnoses; it never guesses.** It reports renamed/stranded Linear projects, unassigned tasks, and labels naming no local project. Ambiguous task moves stay in the plan output for a human.
- **Open question:** `lf pm doctor` and `lf pm sync --plan` are byte-for-byte identical (both call `pm_sync` with `plan: true`). `doctor` earns its keep only as a memorable read-only verb. Collapsing it is a product-surface call, deliberately left to Jack.

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
