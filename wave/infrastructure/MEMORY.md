# infrastructure wave memory

Renamed from `systems` in the 2026-07-08 wave/project/task restructure. Steers Loopflow toward boring releases: nightly verification that never deploys, weekly publishing gated on that same verification, and repo-owned `lf cron` jobs — with Cadenza mirroring the cadence.

## Shipped

- **Install syncs skills** — the repo refresh and local `--use` paths run `lf sync-skills --yes` after installing `lf`, so `~/.claude/skills` and `~/.agents/skills` track the freshly installed binary. Sync failure warns but never fails the install; the binary is already in place. First increment of "one command keeps local fresh."
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

## Gotchas

- **`scripts/test.py --all` cannot green the Loopflow UI suite headlessly** (filed). `xcodebuild` runs 304 app/unit tests to a pass, then `LoopflowUITests-Runner` hangs before establishing its connection and Xcode exits 65. Reproduced with a fresh `derivedDataPath`, so it is not a stale-cache artifact. Treat a `--all` UI failure as unproven, not as a regression, until the runner hang is fixed.
- **Dotted-root vs dotted-ancestry collision — RESOLVED** by the WaveId decoupling: the dir is a flat `.`-chain, the remote branch carries `/`+author, and ancestry is read from the `Run` record, not the string. The old `branch_names.schema` grammar that caused it is gone.
- **Run `cargo test` to completion before trusting a green-looking suite.** A failing lib target makes cargo skip every later target, so lib failures mask bin failures — two `bin/lf.rs` tests naming a deleted command had never run at all.
- **Rust compilation does not validate SQLite column names.** Runtime SQL whose shape depends on a released schema must be shared with a behavior test that prepares and executes it against the materialized migration head. Epoch Work ownership is three exclusive foreign keys (`wave_id`, `project_id`, `task_id`); generic kind/id belongs to explicit routes such as parent Asks, not to Epochs.
- **Source history must reconstruct every applied release frontier** (learned 2026-07-20). One pre-schema-closure local promotion embedded a test-materialized `0.12.4` batch and advanced the shared store while git retained the ten source drafts and omitted the canonical file. Recovery preserved the database, extracted the canonical bytes from the retained immutable binary, matched their checksum to `schema_migrations`, registered the batch, and removed only byte-identical drafts. If a store is ahead by an unknown migration, retain state and old binary bytes; prove the checksum before ratifying history. Since #1123, draft-bearing candidates fail promotion even at an exact frontier, while a schema-complete exact-frontier CLI repair may safely activate with live Runs because it writes no migration.
- **Tests must survive draft migration materialization** (learned 2026-07-21).
  Release-equivalent Rust tests delete ordinal-free drafts and compile the
  generated canonical batch. Test fixtures resolve migration SQL by its draft
  marker through `migration_sql_for_test`; an `include_str!` pointing directly
  at `migrations/drafts/` passes locally and fails the release tree at compile
  time.
- **Ordinary-PR integration tests inherit Task authority inside a worker.** Scrub `LF_RUN_CONTEXT` (plus its lease/invocation companions) when a fixture deliberately represents a non-Task repository. A missing registry while Run context is present is the intended fail-closed behavior, not a commit/push regression.
- **Concurrent editing corrupts a file; concurrent rebasing corrupts history.** Two drivers sharing one worktree shared its `rebase-merge` state dir: conflicts resolved themselves between one command and the next, and `done` advanced 6→22 with no `--continue` from the losing session. Nothing was lost that time. Check for a live agent before working — or rebasing — a wave worktree; the driver that owns the worktree owns its `.git` sequencer.
- **Linear Project names are identity-bearing under the native hierarchy.** The CLI slug derives deterministically from the Project name, and the slug is the cache filename (`projects/<slug>.md`) and the `--project` argument to task commands. Renaming a Linear Project changes its slug, so it moves the cache file and changes every task command's input — a rename is a migration, not a cosmetic edit.
- **Environment configures a process; it must never decide what the process is.** An earlier runtime chose between booting a listener and being a resident from inherited environment, so a promoted wave could attach to its parent's listener with the parent's token. The current `lf wave` surface keeps that role explicit.
- **Current PM truth and durable Work history have different lifetimes** (learned 2026-07-21). A terminal Project omitted from the current PM snapshot can still own non-terminal historical Task Work. Wave reads must render the current PM hierarchy and classify the stranded Project/Task separately as Wave-owned degraded evidence; they must not fail the whole join, delete history, or synthesize a PM Project. Recovery must use the stable Work id (`lf work abandon task <work-id>`) because higher-level Task commands may inspect a historical worktree that no longer exists.

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
  deleted. Authored input is a durable Work Steer. Best-effort process nudges may
  reduce latency, but the server follow-up must make Home-owned Ready scanning
  the correctness path so a stopped Project cannot miss child Feedback.
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
  lifecycle, Work Epoch, and completion event in one transaction. Never mint a
  synthetic Run to reuse a Run-owned terminal transition. Prove this boundary
  with a zero-agent-boundary fixture and repeated reads that count Runs and
  completion events.
- **Phase-owned state needs the same freshness boundary in memory and storage**
  (learned 2026-07-20). Passive reconciliation may advance a durable Task to
  finally while its active Run still holds a pre-final snapshot. Refresh a gate
  proposal only within the same finally epoch; first/loop snapshots keep no
  proposal and the store's `phase_epoch` fence preserves newer durable truth.
  Validation runs before SQL, so a persistence fence cannot repair a torn local
  refresh. Terminal Work remains authoritative over stale resumable failure
  observations.
- **Durable Ask is the only blocking human-input primitive** (decided
  2026-07-21). Interactive Task phases are advisory: the runner makes one launch
  attempt and advances independently of launcher success, UI lifetime, or
  Invocation handback. A launched surface is read-only while the next writable
  phase owns the Task worktree; providers without enforceable read-only mode
  fail closed. Launch failure ends the Invocation once, while a successful
  launch stays live until optional handback records its evidence.
- **Persisted executable references are an installed-state invariant** (learned
  2026-07-21). Removing or renaming a builtin flow requires a forward migration
  for every surviving Task pin, plus catalog resolution before Run reservation.
  A non-empty stored name is not proof that the installed binary and worktree
  can execute it.
- **One Task failure is one atomic durable fact** (learned 2026-07-21). The
  failure event and Run/Invocation terminal state commit together; if the event
  cannot persist, the Run stays open and recoverable. Automatic relaunch is
  progress-relative and bounded, and only durable progress or explicit User
  input resets its budget. An empty Run slot alone never authorizes retries.

## Planning model (settled, PR #852)

- **Three nouns, distinguished by kind, not size.** Wave = durable operating context (memory, cadence, budget, chat, project selection). Project = one measured bet inside exactly one wave, a definition plus KRs. Task = a concrete change. No project trees, no orphan projects.
- **Where each noun lives.** Wave = `wave/<wave>/` (`GOAL.md` + `MEMORY.md`). Project = a Linear Project under that Wave's Initiative. Task = a Linear Issue under exactly one Project. Local Project and roadmap mirrors are deleted; SQLite is a read model, not a second authoring surface.
- **Native Linear hierarchy (shipped, jack-heart/infra): Wave → Linear *Initiative*, Project → Linear *Project*, Task → Linear *Issue*.** Supersedes the label model below. The wave anchors on `pm.linear_initiative`; `lf pm init` creates the Initiative, migrates each legacy `project:<slug>`-labeled issue into a native Linear Project (moving it via `move_item_to_project`), writes `linear_initiative`, and drops `pm.linear_project` **only** once every legacy task carried exactly one recognized label. A task with zero or >1 recognized labels is left behind, `pm.linear_project` is retained, and the `unmigrated` count is reported so a human assigns the label and re-runs `pm init`. Project **definition + KRs live in Linear Project `content`** (`## Definition` / `## KRs` checkbox Markdown), the one-line summary in `description`; Loopflow parses them into typed `PmProject { slug, summary, definition, krs: Vec<PmKr{text,holds}> }` rather than leaking the storage convention. `holds` is a human/loop `[x]` judgment, not derived evidence. A **duplicate or empty derived slug is a hard drift error** (silently choosing one would weaken exactly-one-wave). Seeding is **restart-safe**: `pm init` writes a transient `pm.linear_seed_pending` marker after creating the Initiative and before seeding Projects, resumes only the missing Projects on re-run, and clears the marker on clean completion. Linear permits a Project in many Initiatives; Loopflow enforces exactly-one-*wave* at its own layer and leaves unrelated associations alone. No local Project cache survives as an alternate source of truth.
- **Superseded — label model (PR #852):** one Linear project per wave, Loopflow projects as `project:<slug>` issue labels. Was the incremental-migration bridge; the native hierarchy above replaces it and reads legacy labels only as migration input.
- **Open (native hierarchy):** standing quality-frontier projects have no natural Linear completion date; the API allows date-less projects, so leaving frontier bets undated is a product convention, not a schema blocker — don't force a target date on them.
- **Vocabulary discipline.** Say "Linear project" for the Linear object, "project" for a Loopflow measured bet. No fourth noun — "space" and "provider container" were considered and rejected as user-facing words.
- **`sync --plan` diagnoses; it never guesses.** It reports renamed/stranded Linear projects, unassigned tasks, and labels naming no local project. Ambiguous task moves stay in the plan output for a human.
- **Open question:** `lf pm doctor` and `lf pm sync --plan` are byte-for-byte identical (both call `pm_sync` with `plan: true`). `doctor` earns its keep only as a memorable read-only verb. Collapsing it is a product-surface call, deliberately left to Jack.

## Next

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
- **Installed-upgrade semantic gate** — resolve every active placed Work's
  persisted lifecycle through the candidate builtin and repo-local catalogs
  after migrations, before that binary becomes the Home launcher.
- **Project terminal-receipt parity** — make Project failure events and
  Run/Invocation settlement share the atomic receipt boundary now used by
  Tasks, with a fault-injection proof.
- **Replicate intentionally** — apply the skeleton to Manabot/Hootro only when they need it.

- **Deferred: "up/down 5ths"** (Jack, 2026-07-06) — referent unresolved. `lf wt` shipped up/down stack navigation this branch; candidates for the phrase are stack level-jumps ("fifth" = a level), circle-of-fifths name generation instead of random word pairs, or a chord-model transpose. Jack said "keep going" — deferred, not dropped.

The rebase-efficiency follow-ups are resolved by PR #818: config/naming-schema redesign shipped as `WaveId`; `lf wt create` is sibling-only; Task and Project Work own higher-level worktree placement; land rotation and `next`/`advance` are removed.

### How to judge rebase efficiency (dogfood metrics from `.lf/tmp/metrics/ops.jsonl`)

Local-only JSONL, reviewed weekly. Key product metrics: **agent-rebase rate** (% of rebases launching an agent), **avoidable rebase-agent rate** (stale/empty/generated-only branches that still launched one — target 0), median `land`→queued/merged time, post-land repair rate, and command-drift rate (prompt-recommended commands the installed `lf` can't parse). Then flip one default at a time: stack-by-default `wt create`, stale-empty reset before rebase, land/advance split, generated-only reset policy. Synthetic-workload replay harness (50–100 disposable histories, current vs classifier in trace mode) is unbuilt — file if tuning thresholds needs it.
