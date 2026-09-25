# product wave memory

Renamed from `concerto` in the 2026-07-08 wave/project/task restructure. The wave's
scope widened past the Mac app: product now owns the shared API and every surface
(CLI, Mac, iOS, agent turns, workers). Older notes below still say "Concerto" where
they mean the Mac surface.

## Work and continuity (settled 2026-07-19, Sessions revised 2026-08-30)

- **Work is stable identity, not a process.** Wave, Project, and Task are the
  three Work kinds. A Run is one bounded period of execution authority; a
  AgentInvocation is one provider/process attempt inside it; a Turn is one
  observed provider boundary.
- **Domain structure carries continuity.** A Wave owns `GOAL.md`, `MEMORY.md`,
  cadence, Chat, and its Project portfolio. A Project owns definition, KRs, and
  Tasks. A Task owns its directive, worktree, and serial PR chain. Project and
  Task do not copy parent context or inherit recent Wave conversation.
- **Steer is the one durable authored input.** Chat is its human Wave
  presentation, not a second mailbox or history truth. Radio, agent channels,
  machine bylines, and the database message bus are deleted.
- **Another Work perspective is an ordinary Run; human interaction is a
  Session.** Launch `lf --as <work> : <question>` when another agent perspective
  is useful. `lf ask` is reserved for a human boundary: it starts a provider
  Run in the caller's checkout and blocks until that Session is explicitly
  completed. A declared human Task node persists its exact playhead and advances
  only through Approve or Iterate. There is no agent exchange row, answer lane,
  or dedicated answer controller.
- **Wave memory is file-only.** Applicable ancestor `MEMORY.md` files are read
  oldest-first. There is no live memory stream, and recent Wave Chat is not
  ambient Project/Task prompt context.
- **Environment configures a process; it never decides what the process is.**
  Work identity and Run authority come from durable state, not inherited
  endpoint variables or a surviving terminal.
- **Backlogs are allowed.** Linear Tasks may exist without a Run; open Runs are
  not the Wave's roadmap.

### Runtime boundary still open

- **Missing lifecycle flows must settle, not retry** (dogfood 2026-07-21).
  LOO-167, LOO-193, and LOO-195 repeatedly alternated between `ready` and a
  short-lived Run while `task` was absent from the installed flow catalog,
  producing hundreds of identical resumable failures. The shared action
  surfaces still recommended `resume` or `no_action`, and no non-destructive
  pause exists for ready Task Work. Flow resolution failure must become one
  durable blocked/failed boundary with bounded retry and a legal next action.
- **Containment liveness is not provider-progress proof** (dogfood 2026-07-21).
  LOO-207 and its owning Project reported `process_alive: true` while no exact
  `lf ps` receipt existed and `lf top` recorded no completed output; interrupt
  receipts contained no Turn ids and the residents immediately relaunched.
  Supervisors must distinguish a live containment from an owned provider
  process and an advancing Turn before recommending wait, interrupt, or retry.
- Cross-Work questions need no parent resident or answer lane. The broader Home
  server design remains open for automatic backlog dispatch, remote nudges, and
  Ready work that has no blocking child command to trigger a wake.
- The server design must assign one owner each for dispatch, liveness, retry,
  streaming, and remote nudge before Wave, Project, and Task controls collapse
  onto one host path.
- Mid-turn Steer remains provider-dependent; queued durable Steers must still
  survive provider and app exit.
- Composite flow nodes still use the internal `__flow-step` fallback, and
  Project-loop caps still need real dogfood data before changing.
- Residency still reads Wave definitions from the main checkout; promotion
  authored in a worker worktree requires landing first.

## Model (design invariants)

- `lf` and durable store projections define the product API; Mac and iOS
  consume that model rather than inventing a parallel lifecycle.
- **Terminal-only and agent-embedded Loopflow are normal primary modes.** Chat
  is an optional shared steering and observation surface, never a prerequisite
  for operating Loopflow or an onboarding funnel every user must enter.
- **Wave Chat is asynchronous steering, not a low-latency support chat.**
  Durable inputs preserve order and may wait behind existing Wave work; surfaces
  show delivered, queued, and working state instead of implying an immediate
  conversational reply.
- App surfaces navigate, present, and Steer Work. A view, terminal, provider
  process, or listener is never the source of Work or human-playhead truth.
- A provider session is AgentInvocation continuity, not Work identity.
- Runtime ownership remains deliberately unresolved until the Home/Work server
  topology can explain automatic Ready dispatch, remote nudge, and failure
  recovery in one diagram.

### Charter model (restarted 2026-07-07, resettled 2026-07-08, Linear-owned 2026-07-10)

- **GOAL.md holds the Objective only** — mission/vision/vibe collapsed into one
  `## Objective` paragraph. The Measures (KRs) left the charter. Frontmatter now
  binds a Linear **Initiative** (`pm.linear_initiative`, was `linear_project`);
  product's is `33e774b0-ec3b-4bd6-a4f8-07676f9e897b`.
- **Linear owns the durable middle tier now — `wave/<wave>/projects/*.md` is
  gone (this branch).** The Initiative → its Projects (definition + KRs) → their
  Issues is the sole authoring surface; the machine SQLite registry is a **read
  model**. `lf pm sync` fetches the linked Initiative and atomically replaces the
  wave's snapshot; ordinary reads (`lf pm show`, project selection, agent
  context, the Mac) read the snapshot, never Linear. `lf pm init` links/creates
  Initiatives and writes the binding; `lf pm project create/update` and the
  `lf pm task ...` mutations write Linear then refresh SQLite. KRs still read as
  proof, not backlog bullets.
- **Plan is fully wired end to end (this branch, closed loopflow-api task
  `8e77a60f`).** `PmShowResult` (`ops/pm.rs`) *is* the `--json` wire shape:
  `{wave, provider, initiative, project, synced_at, projects, items}`. Swift
  `PmShowSnapshot`/`RegistryQuery.plan()` decode `synced_at`+`projects` and map
  each Project's definition + KR proof into `WavePlan`; `WaveDetailPane` renders
  them. No `projects/*.md` is written anywhere — sync only ever touches `GOAL.md`
  frontmatter. The earlier "pick one source" blocker is gone: `PmShowResult` is
  the single envelope, so there is no second shape that can omit fields Swift
  needs.
- **`lf pm show` freshness policy (this branch, `--sync`/`--no-sync`).** Auto
  mode reads the SQLite snapshot through a staleness gate keyed on `synced_at`:
  **fresh <1h** serves cache, no network; **soft-stale 1h–1wk** tries one bounded
  refresh (5s cap) and falls back to cache on any failure, saying so;
  **hard-stale >1wk** refreshes or errors (too stale to serve silently). `--sync`
  forces a refresh; `--no-sync` is cache-only. Because every mutation refreshes
  the acting machine's snapshot, single-machine reads run *ahead* of the last
  explicit sync and `--no-sync` is fully current without a network call. Agents
  tolerate failure — drop the PM section rather than block. The scheduled
  `lf pm sync` cron keeps the snapshot warm for cross-machine readers.
- **task = one Linear issue** under a Project. Linear is the only roadmap.
- **Linear Team ownership moves to repository scope in PRD-43 (capability branch,
  2026-07-20).** Repo-only `.lf/config.yaml` owns `pm.provider` plus the stable
  `pm.linear_team`; each `GOAL.md` retains only its Initiative. A managed Team's
  description carries one canonical Git-origin claim marker, validated before
  every networked mutation. This is a cross-machine collision detector, not a
  distributed lock (Linear exposes no compare-and-swap update).
- **PM ownership follows stable provider edges.** Required snapshot fields are
  Project `initiative_ids` + `team_ids` and Task `project_id` + `team_id`.
  Reads and mutations resolve Issue → exactly one Project → exactly one
  Initiative → exactly one local Wave; shared Issue prefixes and
  ancestry-qualified Project titles are presentation only. Rust and Swift pin
  this in the shared `pm_show.json` fixture; legacy payloads invalidate rather
  than silently default.
- **Repository-wide `lf pm reteam` is the only migration path.** It preflights
  every linked Wave before provider writes, expands Project Team sets, moves all
  open and completed Issues by UUID, reconciles durable identifiers, narrows
  Projects, verifies/refills every snapshot, then removes legacy sentinels and
  commits. Any Wave-level Team/provider or repo `linear.team` sentinel blocks
  normal PM/Work mutations while cache-only reads and diagnostics remain.
  PRD-43 deliberately preserves Loopflow's checked-in/live legacy bindings;
  PRD-44 must run the consequential LOO migration from merged main.
- The cache-only product snapshot on 2026-07-21 contains three linked Projects:
  loopflow-api, mac-surface-ux, and auditability. The previously listed
  wave-chat, ios-surface-ux, distributed-computing, and product-performance bets
  are absent; do not select or mutate them unless a later synced snapshot
  reintroduces them. The old Concerto project set (session-lifecycle,
  attention-navigation, wave-conducting, remote-connection, palette) was folded
  into the product portfolio and deleted, not tombstoned.

### The `lf` / Home spine (server topology not yet settled)

- **`lf` is the single command implementation.** Local reads query the durable
  registry directly; CLI and app actions call the same Work operations.
- **There is no agent messaging substrate.** Radio commands, channel identity,
  bus tables, cursors, retention, and subscriptions are gone. Durable Steers and
  Work state replace message delivery as product truth.
- **Company Discord is the canonical human Wave Chat backing when configured
  (settled 2026-07-21).** An inbound human message becomes one durable Wave
  Steer and the Wave reply returns to the same channel. Restart catch-up,
  deduplication, self-echo rejection, and outbound receipts preserve one
  conversation. The active backing and conversation epoch are explicit: local
  and Discord compose never operate simultaneously, and product surfaces must
  not persist a second transcript. This listener is not generalized into
  Project/Task communication and Discord history does not become ambient prompt
  context.
- **Home ownership is the next design.** Decide how Ready scanning, remote
  nudges, live deltas, and replaceable executors fit together before moving
  remaining lfd/Wave-listener behavior.
- Resident crons evaluate in **UTC**, so the product `wave` flow at `0 0 8 …`
  fires 08:00 UTC regardless of host timezone.

## Wave ontology & viewer (built this branch, slice 1)

- Swift `Wave` = **objective** (GOAL.md prose; old `goal`/`metrics: [String]`
  retired) + **projects** (the plan) + **runs** (the ledger). `WavePlan` /
  `WaveProject` in `Loopflow/Models/WavePlan.swift`. **The plan's two halves now
  read from two sources (this branch):** `WavePlanParser.objective(...)` reads
  only `## Objective` from `GOAL.md` (its `projects/*.md` parsing — title, summary,
  KR checkbox proof — was deleted with the files), and
  `RegistryQuery.plan(wave:objective:cwd:)` builds the projects+KRs from
  `lf pm show --json`'s SQLite snapshot. `RepoState` paints the objective
  synchronously, then fills projects from an async `registryQuery.plan` task.
  `WaveDetailPane` splits the surface: plan left, live WaveChat right.
- **`BacklogItem` (this branch)** decodes `id, name, description, rank, completed,
  project, assignee` — matching the item shape `lf pm show --json` actually emits;
  the old `labels: [String]` was dropped for the explicit `project` slug.
- **Vocabulary locked:** *Run* = one bounded period of execution authority plus
  its durable record; *AgentInvocation* = one provider/process attempt;
  *Session* = a provider-native conversation or unresolved human boundary;
  *Work* remains Wave, Project, or Task. *Exec* remains process evidence, not a
  fourth Work kind or Session identity.
- **Plan render works end to end now** — `PmShowResult` carries `projects` +
  `synced_at`, `RegistryQuery.plan`'s `PmShowSnapshot` decodes them, and
  `WaveDetailPane` shows each Project + KR proof. The old decode-throws blocker is
  closed (see the charter section).
- The server follow-up must decide live Run/Turn streaming and remote plan
  queries without introducing another lifecycle.

## Swift data path — RegistryQuery is the single reader

- **All data reads converge on `RegistryQuery`** (subprocess `lf … --json`,
  daemon-less) — including Waves, status, roadmap, Sessions, Activity, usage,
  and recent Runs. The
  HTTP-to-lfd-as-API path is **deleted**: `LocalWaveService` (~1500 lines) and
  `WaveServiceProtocol` are gone; ~22 consumers rerouted onto RegistryQuery.
- **`RunStatus` biases to `lf`** — align to `lf`'s lowercase tokens (`running`,
  `ok`, `waiting`, `failed`, `pending`), not the lfd int enum. No invented
  `cancelled`. An unknown status must be **loud** (surface it), never a silent
  `?? .pending`. When `lf` and lfd disagree, `lf` wins.
## Performance — reads never block on lfd

- **Governing invariant: the repo/wave list paints from `lf` (daemon-less); a
  listener or Home process must never gate a read.** First instance
  (diagnosed, fix implemented this branch): `WavesView.syncRepoStates`
  early-returned while `SharedDaemon.currentConnection == nil` and
  `prepareConnectionIfNeeded` awaited `SharedDaemon.manager.start()` — the wave
  list waited on lfd booting even though `RegistryQuery`/`lf ls` is daemon-less.
- Cheap wins landed this branch: one machine-wide `RegistryQuery.allWaves()`
  (`lf ls --json`) per poll distributed to each `PortfolioRepoState` (was one
  spawn per repo); memoized `lf` binary resolution (was `lf help wave` probe per
  query); first paint boots lfd concurrently, not as a barrier; `WavePlanParser`
  moved off render/`body` onto a per-refresh cache; one `tmux list-sessions`
  snapshot + Set lookup (was `tmux has-session` per wave). See the Performance
  project for the ranked audit; budgets/instrumentation not yet built.

## Sessions projection and native resume (settled 2026-08-30, PR #1250)

- **`lf session list --json` is the sole unresolved-human-work projection.** It
  merges ordinary interactive TUI Runs, ad-hoc human Asks, and Task human
  FlowSteps into required-field `SessionRecord` values. The Mac app renders that
  projection and owns no second queue, title store, liveness model, or resolution
  state.
- **Provider history is the resume authority.** Opening a Session stops only the
  exact Loopflow-owned client whose PID and birth evidence match its Run, then
  resumes through Codex, Claude, or OpenCode's native command. Closing a pane or
  provider exit resolves nothing. Complete applies only to interactive Sessions
  and ready Asks; Approve and Iterate apply only to ready Task FlowSteps.
- **tmux is only the first client's detached PTY cradle.** It lets a human-bound
  TUI start before a desktop exists, but it is not Session identity, readiness,
  presentation, liveness authority, or a resolution mechanism. A kernel advisory
  lock protects initial publication until provider history and the exact owned
  client are observable, so app open cannot race startup into a duplicate Run.
- **The Mac multiplexer presents Sessions without owning them.** Selecting a row
  replaces the focused Ghostty pane or focuses the pane already showing that
  Session. Completing the selected Session removes its row and reconciles the
  pane to the useful empty workspace. Repository selection uses canonical Git
  common-directory identity so linked Task worktrees never appear as portfolio
  roots.
- **Work conditions remain non-actionable descriptions.** `lf status` and `lf
  roadmap` expose one Task condition: `clear`, `waiting`, `blocked`, or `unknown`.
  Sessions alone open or resolve human work. NOW groups the same conditions, and
  `lf usage` accepts the Wave/Project/Task drill shared by `lf runs` rather than
  inventing another hierarchy.
- **Two configured-path proofs remain release evidence.** One promoted Ask must
  survive open, provider-native continuation, Ready, Complete, pane clearing, and
  blocked-caller release. The permissioned macOS UI gate must interact with a
  Session and prove focus/action/pane reconciliation. A mocked `SessionRecord`,
  launch-only screenshot, empty list, or live row without caller release is not
  equivalent evidence.
- **Keep the next projection reduction coherent.** Rust already owns Session
  legality and exact Work identity, while Swift still reconstructs legal actions
  from kind/state and joins the roadmap for display labels. If that surface grows,
  project legal actions and display Work path in `SessionRecord`, then delete the
  Swift action matrix, replacement-policy inference, Sessions-only roadmap join,
  and unused narrower scopes together.
- **PM reconciliation is queued after Linear reconnect.** The cached LOO-251
  directive still describes an Ask-specific `lf ask list --user` and tmux-shaped
  surface. Rename it to “Finish the native Sessions multiplexer and promoted Ask
  handoff,” replace its contract with the model above, and keep the two configured-
  path proofs as its closure criteria. File the projection reduction as a
  separate Mac Surface UX Task after PR #1250 lands.

## Patterns (verified 2026-06-30, remote TLS connection)

- **Concerto reaches remote `lfd` over HTTPS via Tailscale, not TLS in `lfd`.**
  `deploy/tailscale-lfd-host.sh` keeps native lfd on `127.0.0.1` and runs
  `tailscale serve` as HTTPS ingress with a real `*.ts.net` cert;
  `deploy/native-lfd-host.sh` owns launchd. Keep TLS termination outside lfd.
- **Remote bearer token read fresh from `~/.lf/concerto.yaml` per request.**
  `RemoteConnectionConfig` (`swift/LoopflowCore/Config/ConcertoConfig.swift`)
  carries optional `token`; `ConnectionStore.token(for:)` prefers it over
  static/Keychain, but only when config host+port match the active connection
  (no cross-profile leak). Rotation is immediate. `configLoader` is `@escaping`,
  held on the store so the read stays live.
- **CA-trusted certs (incl. `*.ts.net`) use system trust, not pinning.**
  `CertificatePinningDelegate` skips pinning for CA-trusted chains.
- **Dev builds use bundle id `com.loopflow.concerto.dev`** — `scripts/concerto-dev.py`
  rewrites the assembled `Concerto Dev.app` Info.plist so worktree runs don't
  clobber installed-app remote settings.
- **macOS UI-test mode skips bundled daemons + remote subscriptions** (guard in
  `WavesView`/`ConnectionStore`); UI tests must not touch a live remote host.

## Not yet built

- Remote connection: no multi-profile config (schema is one remote `connection` +
  optional container settings — speculative with a single Mac mini host); no
  live-tailnet CI round-trip (coverage is script syntax + Config/ConnectionStore
  tests); no bundled TLS inside lfd (rejected alternative, not a gap).

## Wave controls & truthful failures (built this branch, `wave-controls`)

The 2026-07-10 dogfood exposed four independent surface failures; all repaired
here on top of PR #849's signed-test/release hardening.

- **Stop is a wave lifecycle verb: `lf stop <name>`.** Top-level command (not
  `lf loop stop`), closing the old "no single-wave stop" gap. It discovers the
  live loopback listener via the same `.wave-endpoint` `lf serve` writes, posts
  `POST /stop`, and waits briefly for graceful shutdown. Missing/stale endpoint
  = idempotent success ("already stopped"). **The listener is the sole cleanup
  owner** (`run_listener`): stop supervisor → terminate resident → deregister
  runtime registration → remove only this boot's endpoint + resident-token
  files. Detached
  worker loops stay independent; the listener never owned their tmux. The Mac
  Stop button shells through the same CLI verb via `LocalWaveAgentLauncher`
  (launcher tests pin the exact `lf stop <wave>` argv) — one implementation, CLI
  and GUI. **The agent exec door denies `stop`** (`ExecVerdict::Deny`) so a
  worker can't tear down its steward wave.
- **Empty `Thought` records never become cards.** Whitespace-only thoughts are
  dropped at the listener's shared turn-item boundary (clean new journals) AND
  filtered in the shared Swift model (existing journals replay clean). Non-empty
  thoughts and every other item type survive.
- **Transcript follows only while the reader is at the bottom.** A near-bottom
  flag derived from scroll geometry gates auto-follow; scrolling back disables
  it, returning to the bottom re-enables. Initial replay starts in follow mode.
  No timer, no buffered-copy model — it tracks reader intent only.
- **Failed bodies are attempts, not failed waves** (`AttemptFailurePresentation`).
  A surface-only projection over existing provenance; runtime/journal/wire
  unchanged. Key: `StepKey = (invocation_id, step_index, iteration)`. A
  body-backed failed turn retains its exact `termination_reason`; a later
  different body with the same key ⇒ `retrying` (running) / `recovered on retry`
  (complete); same step still selected, no active body, loop not failed ⇒
  `retry pending`; else `Attempt failed`. Bodyless failed turns keep the neutral
  `Turn failed` fallback. **Never infer terminal step or wave failure from an
  attempt** — the capacity-error receipt now reads `Attempt failed · recovered
  on retry` with the reason visible, and the successful retry is its own turn.
- **Dictation is Wispr Flow (Mac + iOS), not a built-in.** The product owner
  chose Wispr Flow, so the unused `VoiceInputService` (~1276 lines), WhisperKit
  package, its tests (~554 lines), and microphone permission declarations left
  the product rather than being carried into the signed build.
- **Signed UI-test gate reconciled with CI:** PR #849's signed macOS
  `xcodebuild build-for-testing` compiles the visible controls without requiring
  hosted Automation permission; executing UI tests stays an explicit
  host-permissioned action (macOS Automation can stop the runner pre-bootstrap).

## Learnings

- **Reshape proven code; don't rebuild beside it** (code only — a rewrite loses
  hard-won correctness). Does NOT apply to the charter: stale framing is a
  liability, so GOAL.md/roadmap get rewritten freely while MEMORY is curated. The
  fresh `RepoSidebarWindow` re-derived the burgundy sidebar / create sheet /
  terminal panes and got each subtly wrong; the proven components already encode
  the right style + behavior — adapt them.
- **Burgundy sidebar = a custom `VStack{…}.background(Color.loopflowBurgundy)`
  with white text** (`WaveSidebar.swift`), NOT a `NavigationSplitView` column (its
  gray vibrant material can't be overridden). Fields = `.textFieldStyle(.plain)` +
  `palette.surfaceMuted` (`CatchWaveView`), NOT `.roundedBorder` (renders black).
- **`loopflow-dev.py` builds from the worktree it runs in.** Run it from the
  branch checkout. Repository discovery collapses linked worktrees to the
  canonical main checkout through the Git common directory; Task Work remains
  the only surface that presents its worktree.
- **Human-bound provider clients resume natively.** Reuse `SessionRecord` and
  `lf session open`; do not restore lfd terminal attachment, a tmux presentation
  path, or Ask-specific Swift plumbing.
- The high-value review move was catching invented fields that duplicate existing
  ones (e.g. `RunStatus`), not re-litigating the approach.
- `cargo test -p loopflow dto_fixtures` filters by test name; use
  `--test dto_fixtures` to run that integration file. Headless runs set
  `LF_RUN_ID`; Rust tests asserting generated journal ids / branch-derived ingest
  must clear it or full `cargo test -p loopflow` fails only under agent runs.
- Kickoff line numbers drift fast — re-verify before citing in a design.
- **Migration numbers collide across branches — shared `lfd.db` is the blast
  radius.** Product and intelligence both minted `061` (`061_pm_snapshots` vs
  `061_trace_capture`); distinct version strings apply but inter-order is
  undefined. Worse, editing a historical migration CREATE in place (product added
  `run_events.context` to `057`) means DBs created before the edit never get the
  column, and `057` won't re-run — so `validate_run_events_schema` selecting
  `context` takes down *every* command sharing `lfd.db` (that was the `pm show`
  break; worked around by hand-adding the column). Fix is intelligence's, one
  line: `061_trace_capture`'s unguarded `ALTER TABLE run_events DROP COLUMN
  context` fails `no such column` on pre-context DBs and isn't in the convergence
  path — tolerate it (or rebuild). Product must NOT add a forward `ADD COLUMN
  context`; it would fight the drop. Wants a real convention: per-wave migration
  ranges, or Jack's idea — a separate dev lfdb via `LF_HOME=~/.lf-dev` (honored at
  `lfd/mod.rs:66`) so in-flight schema can't corrupt the real ledger.
