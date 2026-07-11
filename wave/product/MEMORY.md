# product wave memory

Renamed from `concerto` in the 2026-07-08 wave/project/task restructure. The wave's
scope widened past the Mac app: product now owns the shared API and every surface
(CLI, Mac, iOS, agent turns, workers). Older notes below still say "Concerto" where
they mean the Mac surface.

## Minds (settled 2026-07-10, the `minds` branch)

The execution model, in the four verbs it produced. `scratch/minds.md` is the
long form and dies at land; this is what survives.

- **Waves are the only minds.** A mind is a *read set*, not a process: it
  re-reads the live thread and memory. Task and project loops are hands —
  private transcript, public posts, no memory of their own. The only way to make
  another mind is `lf project promote`, which grants residency (a process, an
  endpoint, a cadence, a budget) and costs the parent its ambient overhearing.
- **The verbs, and why they are four:**
  | `lf serve <wave>` | boot a mind: listener, thread, playhead. Steerable. |
  | `lf loop <flow> <seed>` | run a bounded child loop to its bit. Batch. `--detach` is the concurrency switch; `--dispatch` is deleted. |
  | `lf chat` | humans only: converse with a served mind's thread. HTTP/SSE on the listener. |
  | `lf radio pub` / `lf radio sub` | agents only: publish/subscribe on the bus. Publish is an INSERT; no server in the path. |
  `lf __resident` is hidden. **Environment configures a process; it must never
  decide what the process is** — the earlier `lf loop <name>` chose between
  booting a listener and being a resident by reading `WAVE_SERVER_ENDPOINT` from
  the environment, and tmux hands a promoting pass's env straight to its child,
  so a promoted wave's resident attached to its *parent's* listener with the
  parent's token. Split the verb by what it does.
- **Inhabiting is a call, not a mode.** The wave LLM runs `lf loop project "…"`
  and blocks; nothing about the wave changes. Blocking is one long tool call —
  the thread's tool-boundary ear is traded for the inner loop's pass-boundary
  ear. A live child loop is presence and extends the pass lease.
- **Delegate all but one.** Keep the project whose next move needs the wave's
  memory and chat in the room — the one you could not write a self-sufficient
  seed for. Not the most important one: inhabited work advances at wake cadence,
  delegated work advances continuously, so keeping the priority starves it.
- **A session is not a conversation.** One thread per wave, journal-backed,
  durable, assembled from many disposable bodies. The human attaches to the
  running pass itself; the persona rotates with the flow's skills and that is
  honest. Rendering each active session as its own chat is exactly how this
  design gets undone in the UI after being right in the runtime.
- **The playhead.** The mind is always mid-flow: a cursor through the wave
  flow's default cycle, wrapping at the end. Enqueue attaches to the *innermost*
  active invocation frame and drains FIFO before returning to the caller — so
  the queue is per-frame, not one flat list. Skip interrupts the body, journals
  a boundary, advances one step; it never restarts the flow. The journal is the
  queue's source of truth, keyed by invocation id + step path (names alone can't
  reconstruct the stack when a flow appears twice).
- **The db IS the bus** (the same move that made the db the registry).
  `bus_messages` + `bus_cursors`; publish is an INSERT, subscribe is a forward
  poll from a rowid cursor, the sweeper rides every read and write with a 1 h
  wall-clock window. No broker, so publishing works with zero loopflow processes
  running and two detached hands hear each other with no served wave. The bus is
  **not a log** — that temptation is the failure mode to guard.
- **Byline is testimony; channel is evidence.** With no server in the publish
  path, client-submitted attribution is the only kind possible: a forged byline
  is visible as a mismatch against the arrival channel, not prevented. Per-hand
  tokens stopped being needed. A mind skips rows bylined with its own channel,
  or steering a hand wakes the steerer with its own steer.
- **Bus delivery is at-least-once, and no doc may promise more.** The listener
  journals a report then commits its cursor; a crash in that seam replays one
  row. Deliberate — a duplicated report is cheaper than a silent lost one. A
  clean restart replays nothing.
- **Nothing crosses a boundary unless someone wrote it down on purpose.** Memory
  is lexical scope (a child reads the parent's live memory, writes only its own);
  chat is a mailbox that stops at the wave. Raw records stay home; authored
  statements travel. That is what lets log-as-truth survive nesting.
- **A hand's ear is the wave's thread**, at pass granularity — every pass is a
  fresh process that re-reads the wave's live memory and chat at birth. Verified:
  no `env_clear` in the pass-spawn path, and `LoopRun` worktrees are wave-named.
  The tmux door is now read-only; it existed only because the mind on the other
  side went deaf at birth.
- **Backlogs are allowed.** Agents file tasks without running them; `loop/README`'s
  "no backlog" is resolved in Linear's favor. What it bought — "the open runs ARE
  the wave's open tasks" — is gone: a task now has three states (filed, running,
  merged), and nothing else prevents a tracker filling with intent nobody does.

### Minds: what did not land

- **A detached loop's driver holds no subscription.** `lf radio pub -c <hand>`
  reaches live `lf radio sub` listeners and nobody else; steering a hand means speaking
  on the wave's thread. On a store bus the fast path is a poll cursor in the
  driver's pass boundary — cheap to build, or to skip deliberately. Open fork.
- **Mid-turn steer is Codex-only.** Claude and OpenCode queue to the next body.
  Vendor-gated; the product question lives in the wave-chat Linear Project.
- **Composite flow nodes** (`and`/`or`/`xor`/`loop`) still run through the
  internal headless `__flow-step` fallback rather than first-class playhead
  frames. Do it when the Mac's breadcrumb starts lying about nested flows.
- **Project-loop caps** still inherit the generic 8-pass / 2-hour task defaults.
  Needs one real project loop's dogfood data, not a guessed weeks-scale timeout.
- **Foreground/background label** on Active sessions: the run ledger doesn't
  persist the owner, so the Mac shows pass/worktree/liveness and declines to guess.
- **PM label removal on promotion:** the provider abstraction has no remove-label
  op, so promotion records residual `project:<slug>` labels instead of clearing
  them. Provider-level API work.
- **Residency reads wave definitions from the main checkout.** Promotion authored
  in a worker worktree stops with an explicit land-before-residency error rather
  than launching a child against files the listener cannot see.

## Model (design invariants)

- Frame, don't render: no native chat UI, and the CLI stays the source of truth — Concerto composes around it.
- Concerto owns wave *navigation* (which wave to open); workflows owns wave *governance* (grading, rollups, rhythm).
- The vendor-session launch mechanism (`vendor-session-launch`) lives in `workflows`; Concerto consumes it.
- lfd owns the goal-loop harness runtime; Concerto attaches to and frames the session, it does not own the loop.

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
- The seven live bets (Linear Projects under the product Initiative): loopflow-api,
  wave-chat, mac-surface-ux, ios-surface-ux, distributed-computing,
  product-performance, auditability. The old Concerto project set
  (session-lifecycle, attention-navigation, wave-conducting, remote-connection,
  palette) was folded into these and deleted, not tombstoned.

### The `lf` / lfd / pubsub spine (direction, started not complete)

- **`lf` is the single implementation.** It queries lfdb directly (daemon-less
  local reads) and runs commands. Primitives already on the Rust side: `lf runs`
  (`lf/commands/runs.rs`, RunSummary + event-folding), `lf radio sub`
  (`lf/commands/sub.rs`, pubsub), lfd exec door (`http/routes/exec.rs`).
- **lfd demotes to proxy + pubsub** — proxies `lf` over HTTP so remote looks like
  local, and streams new runs. It is NOT how things execute, NOT a parallel impl.
  Concerto's bundled lfd earns its keep solely as the pubsub pipe feeding the ledger.
- **Superseded on the agent side:** `lf radio sub` no longer opens a socket — it
  polls the store bus by channel prefix. Its SSE follower lives in
  `lf/commands/thread.rs` and backs `lf chat --follow` — the sole human-thread
  surface now that `lf wavechat` is removed (asserted absent in `lf/mod.rs`; the
  old `chat`+`sub` fusion this design split is gone, no alias). HTTP/SSE remains
  the *thread's* transport (`lf chat`, the Mac); the bus never had a server in
  its path.
- **The agent bus is one explicit namespace (this branch, `lf radio: make agent
  bus operations explicit`).** `lf radio pub [TEXT] [-c NAME | --parent] [--from
  NAME]` and `lf radio sub [CHANNEL] [--json]`; bare `lf radio` prints subcommand
  help. The old top-level `lf sub` and `lf radio TEXT` spellings were removed with
  no alias — a hidden always-failing parser branch reserves top-level `sub` so the
  external-skill fallback can't reinterpret it as a skill name. Transport, cursor,
  prefix, and byline behavior are unchanged; command ownership only. Every builtin
  prompt, webhook argv, doc, and test moved to the one grammar in the same build.
  Resident crons evaluate in **UTC**, so the product `wave` flow at `0 0 8 …` fires
  08:00 UTC regardless of host timezone.
- **Why:** one implementation at the daemon layer, matching "keep one
  implementation"; kills the two-code-path / three-mirror drift the DTO rule fights.
- **This branch is Swift catching up.** Scope boundary: redo Swift to match; do
  NOT migrate lfd's remaining executor into `lf` (its own effort).

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
- **Vocabulary locked:** *Run* = ledger entry (reuses lfd's existing `Run` DTO);
  *session* = a live run's attachable tmux (`/attach`, `TerminalSession`); *exec*
  retired from the frontend (stays loop's word for how a run is born).
- **Plan render works end to end now** — `PmShowResult` carries `projects` +
  `synced_at`, `RegistryQuery.plan`'s `PmShowSnapshot` decodes them, and
  `WaveDetailPane` shows each Project + KR proof. The old decode-throws blocker is
  closed (see the charter section).
- Not yet built: runs ledger renderer, live pubsub wiring, remote/`lf loop show`
  plan query (slices 2–4).

## Swift data path — RegistryQuery is the single reader

- **All data reads converge on `RegistryQuery`** (subprocess `lf … --json`,
  daemon-less) — `waves()`, `status()`, `recentRuns()`, `allWaves()`. The
  HTTP-to-lfd-as-API path is **deleted**: `LocalWaveService` (~1500 lines) and
  `WaveServiceProtocol` are gone; ~22 consumers rerouted onto RegistryQuery.
- **`RunStatus` biases to `lf`** — align to `lf`'s lowercase tokens (`running`,
  `ok`, `waiting`, `failed`, `pending`), not the lfd int enum. No invented
  `cancelled`. An unknown status must be **loud** (surface it), never a silent
  `?? .pending`. When `lf` and lfd disagree, `lf` wins.
- **Known debt (deferred, needs a human call):** `WaveService` remains a ~600-line
  retired-lfd-HTTP facade whose ~25 action methods `throw unsupported(...)` — NOT
  dead, RepoState/SessionState/AuthProviderStore still call them behind live UI
  actions (stop/delete/land/next/addTrigger/combinePRs, session
  create/attach/cancel). Collapsing it is a behavior change under the
  mac-surface-ux bet, not a compress edit. Its
  dict-based `parse*FromJSON` (~260 lines) is a SECOND wire mirror of the types
  RegistryQuery decodes via Codable — the DTO drift hazard — but it backs the
  `session.json` fixture test + ContractTests/WaveTests/AttentionStoreTests;
  consolidating means migrating those onto RegistryQuery's Codable path first.

## Performance — reads never block on lfd

- **Governing invariant: the repo/wave list paints from `lf` (daemon-less); the
  bundled daemon is pubsub-only and must never gate a read.** First instance
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

## Patterns (verified 2026-05-19, embedded-terminal)

- **lfd terminal provenance is `TerminalSession.source`; provider display is
  `TerminalSession.agent`.** Rust `rust/loopflow/src/lfd/types/terminal_session.rs`,
  Swift `swift/LoopflowCore/Models/TerminalSession.swift`. Don't add `interactive`
  or `provider` synonyms.
- **Source constants:** wave-run tmux is `"wave_step_tmux"` (`TMUX_TERMINAL_SOURCE`);
  palette launches use `"palette"` (`PALETTE_TERMINAL_SOURCE`).
  `is_tmux_backed()` treats both as attachable. Persistence has SQLite + Postgres
  mirrors (`lfdb/sqlite.rs`, `lfdb/postgres.rs`, explicit column lists); new
  columns need schema work, new `source` values don't.
- **Attach contract is the shared path.** `POST /v0/terminal-sessions/{id}/attach`
  returns `TerminalConnectionInfoDto {session_name,host,cwd,status}`. Swift panes
  call `RepoState.attachTerminalSession(_:)` and attach Ghostty to the returned
  tmux session; never recreate a parallel client-side tmux name. Attach by session
  id even when the row is lfd-terminal — a succeeded palette row means the flow
  exited into a shell, not that tmux is gone.
- **Palette create path** (experiment): `POST /v0/terminal-sessions`
  `{wave_id,flow,worktree,agent}` → `{session,connection}`; executor builds
  `lf <flow> --no-direction … -w <wave> -m <agent>`, `source="palette"`.
  Lifecycle completion is exit-file based (`.lf/tmp/terminal-sessions/<id>.exit`),
  then keeps the pane alive via `exec "${SHELL:-/bin/zsh}"`; startup reconcile
  re-arms watchers. `PaneConfig` is durable identity only (`terminalSessionId`);
  `launchCommand`/config-normalization removed 2026-05-19.
- **DTO fixtures cover terminal sessions** (`tests/fixtures/dto/terminal_session.json`,
  `create_terminal_session_request.json`; Rust/Swift/Python).
- **lfd readiness probe is `http://127.0.0.1:2486/health`** (unauthenticated root
  route); most API lives under `/v0`, and `/status` is root + auth-protected.

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
  session → remove only this boot's endpoint + resident-token files. Detached
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
- **`concerto-dev` builds from the worktree it's run in** (`REPO_ROOT`); run it
  from the branch's worktree. Repo list is worktree-aware — collapse to main via
  `git rev-parse --git-common-dir`, never present a worktree; default source `~/src`.
- **Wave-agent (`/goal`) launch + attach already exists:** backend
  `launch_wave_agent_session` starts the goal-loop agent in tmux; Concerto attaches
  via `attachSession` → `GhosttyTerminalView` → `tmux attach-session`
  (`TerminalWorkspaceView`). Reuse it; don't add new plumbing.
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
