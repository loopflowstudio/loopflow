# product wave memory

Renamed from `concerto` in the 2026-07-08 wave/project/task restructure. The wave's
scope widened past the Mac app: product now owns the shared API and every surface
(CLI, Mac, iOS, agent turns, workers). Older notes below still say "Concerto" where
they mean the Mac surface.

## Model (design invariants)

- Frame, don't render: no native chat UI, and the CLI stays the source of truth — Concerto composes around it.
- Concerto owns wave *navigation* (which wave to open); workflows owns wave *governance* (grading, rollups, rhythm).
- The vendor-session launch mechanism (`vendor-session-launch`) lives in `workflows`; Concerto consumes it.
- lfd owns the goal-loop harness runtime; Concerto attaches to and frames the session, it does not own the loop.

### Charter model (restarted 2026-07-07, resettled 2026-07-08)

- **GOAL.md holds the Objective only** — mission/vision/vibe collapsed into one
  `## Objective` paragraph. The Measures (KRs) left the charter.
- **`projects/*.md` holds the durable middle tier** — one file per live bet, a
  title + `## KRs`. `ls projects/` *is* the roster of live bets. No status field;
  a dead bet is **deleted** (git is the tombstone). Crons live on the wave only
  (the one heartbeat), never per-project. KRs read as proof — observable end
  states, not backlog bullets or implementation receipts.
- **task = one Linear issue**, labeled `project:<slug>` to attach it to the bet
  it advances. Linear is the only roadmap.
- The seven live bets after the wave/project/task restructure: loopflow-api,
  wave-chat, mac-surface-ux, ios-surface-ux, distributed-computing,
  product-performance, auditability. The old Concerto project set
  (session-lifecycle, attention-navigation, wave-conducting, remote-connection,
  palette) was folded into these and deleted, not tombstoned.

### The `lf` / lfd / pubsub spine (direction, started not complete)

- **`lf` is the single implementation.** It queries lfdb directly (daemon-less
  local reads) and runs commands. Primitives already on the Rust side: `lf runs`
  (`lf/commands/runs.rs`, RunSummary + event-folding), `lf sub`
  (`lf/commands/sub.rs`, pubsub), lfd exec door (`http/routes/exec.rs`).
- **lfd demotes to proxy + pubsub** — proxies `lf` over HTTP so remote looks like
  local, and streams new runs. It is NOT how things execute, NOT a parallel impl.
  Concerto's bundled lfd earns its keep solely as the pubsub pipe feeding the ledger.
- **Why:** one implementation at the daemon layer, matching "keep one
  implementation"; kills the two-code-path / three-mirror drift the DTO rule fights.
- **This branch is Swift catching up.** Scope boundary: redo Swift to match; do
  NOT migrate lfd's remaining executor into `lf` (its own effort).

## Wave ontology & viewer (built this branch, slice 1)

- Swift `Wave` = **objective** (GOAL.md prose; old `goal`/`metrics: [String]`
  retired) + **projects** (the plan) + **runs** (the ledger). `WavePlan` /
  `WaveProject` in `LoopflowCore/Models/WavePlan.swift`; `WavePlanParser`
  (`Services/WavePlanParser.swift`) maps `GOAL.md` (`## Objective`) +
  `projects/*.md` (title + `## KRs` list) into the plan. `WaveDetailPane` splits
  the surface: plan on the left, live WaveChat on the right.
- **Parser recognizes the exact committed grammar** — `## Objective`, `## KRs`
  headings with markdown list items. Change the file shape ⇒ update
  `WavePlanParserTests` first.
- **Vocabulary locked:** *Run* = ledger entry (reuses lfd's existing `Run` DTO);
  *session* = a live run's attachable tmux (`/attach`, `TerminalSession`); *exec*
  retired from the frontend (stays loop's word for how a run is born).
- **Slice 1 is local & file-first** — reads the wave dir directly, no new wire
  shape. Plan renders only when local files exist; remote/iOS/authorless waves
  fall back to WaveChat-only. Projects are modeled with `tasks`, left empty in
  Swift. The per-project split now exists on the CLI side (PR #852: tasks carry a
  `project:<slug>` Linear label, `lf op pm show --project <slug>` filters by it);
  wiring it into `WaveProject.tasks` is unbuilt, and the Swift side must read the
  label rather than invent its own grouping.
- Not yet built: runs ledger renderer, live pubsub wiring, Linear task loading,
  remote/`lf wave show` plan query (slices 2–4).

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
- Product gap (belongs to the infrastructure wave): **no
  `lf wave stop <name>`**. Killing out-of-band means raw `tmux kill-session` or
  `lf op reset-waves` (kills every `lf-*` session — too broad); a wave killed
  out-of-band leaves its registry row `status: running` forever, only the probed
  LIVE column tells the truth. Wants a single-wave stop verb that kills the
  session, clears `.wave-endpoint`, and settles the registry status.

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
