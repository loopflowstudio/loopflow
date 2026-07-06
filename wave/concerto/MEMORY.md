# Concerto wave memory

## Model (design invariants)

- Frame, don't render: no native chat UI, and the CLI stays the source of truth — Concerto composes around it.
- Concerto owns wave *navigation* (which wave to open); workflows owns wave *governance* (grading, rollups, rhythm).
- The vendor-session launch mechanism (`vendor-session-launch`) lives in `workflows`; Concerto consumes it.
- lfd owns the goal-loop harness runtime; Concerto attaches to and frames the session, it does not own the loop.

## Progress — terminal-first rebuild (2026-07-02)

- **The surface is being reshaped, not rebuilt.** First attempt shipped a fresh
  `RepoSidebarWindow` (repo sidebar + wave list + `CreateWaveSheet` launcher;
  commits `b411c714`/`5237df25`/`9bb44534`) — but rebuilding fresh reintroduced
  regressions the proven components already solved (gray `NavigationSplitView`
  sidebar vs. burgundy; black `.roundedBorder` fields). **Decision: reshape the
  battle-tested `PortfolioWindow` into that shape (rename → `WavesView`), reusing
  `WaveSidebar`/`WaveRow`/`TerminalWorkspaceView`, and DROP the fresh
  `RepoSidebarWindow`.** Reuse over rebuild.
- **Then trim.** Actively delete every `Platform/macOS/Views/*.swift` not reachable
  from `WavesView` — delete-until-`xcodebuild`-green is the arbiter. The old
  wave-workspace / multiplexer / native-chat stack goes.
- **A2 (Wave.repo → repos:[RepoWork]) — shipped, all 7 steps.** `RepoWork` type +
  `wave_repos` table + store methods (mirror `wave_crons`), repos stitched on
  read/write, executor resolves worktrees/dispatch from `RepoWork`, status/
  iteration rolled up over repos, nested in the DTO + mirrored to Python/Swift/
  fixtures, and the flat `Wave.repo`/`status`/`iteration` columns dropped. The
  single-repo bridge is now `Wave::repo()` (`repos.first()…`); repo-filters use
  `repos.iter().any(...)`. Ontology: `WaveRun`→`Run` (snapshot flattened),
  `WaveRunStatus`→`RunStatus`.
- **UX research loop** — `lf ux-research` (`propose→generate→evaluate→learn`),
  `scratch/ux-research/`. Loop-01: candidates that beat the plain repo-list ranked
  by *attention* (`failed→waiting→running→idle`) and carried each wave's *reason*;
  tension = glanceable dashboard vs. `⌘K` keyboard-first. Loop-02 queued.

### Learnings (this rebuild)

- **Reshape proven code; don't rebuild beside it.** The fresh view re-derived the
  burgundy sidebar, the create sheet, and the terminal panes — and got each subtly
  wrong. The old components (`WaveSidebar`, `CatchWaveView`, `TerminalWorkspaceView`)
  already encode the right style + behavior; adapt them.
- **Burgundy sidebar = a custom `VStack{…}.background(Color.loopflowBurgundy)` with
  white text** (`WaveSidebar.swift`), NOT a `NavigationSplitView` column (its gray
  vibrant material can't be overridden). Fields = `.textFieldStyle(.plain)` +
  `palette.surfaceMuted` (`CatchWaveView`), NOT `.roundedBorder` (renders black).
- **`concerto-dev` builds from the worktree it's run in** (`REPO_ROOT =
  __file__.parent.parent`); run it from the branch's worktree. Window title shows
  the target repo.
- **Repo list is worktree-aware** — collapse to main via `git rev-parse
  --git-common-dir`; never present a worktree. Default source `~/src`.
- **Wave-agent (`/goal`) launch + attach already exists:** backend
  `launch_wave_agent_session` starts the goal-loop agent in tmux; Concerto attaches
  via `attachSession` → `GhosttyTerminalView` → `tmux attach-session` (see
  `TerminalWorkspaceView`). The wave screen reuses this, not new plumbing.

## Patterns (verified 2026-05-19, embedded-terminal implementation)

- **lfd terminal provenance is `TerminalSession.source`; provider display is
  `TerminalSession.agent`.** Rust struct at
  `rust/loopflow/src/lfd/types/terminal_session.rs`, Swift mirror at
  `swift/LoopflowCore/Models/TerminalSession.swift`. Reuse these fields for
  embedded terminal lifecycle/provider UI; do not add `interactive` or
  `provider` synonyms.
- **Tmux-backed lfd sessions use source constants.** Existing wave-run tmux
  source is `"wave_step_tmux"` (`TMUX_TERMINAL_SOURCE`); palette launches use
  `"palette"` (`PALETTE_TERMINAL_SOURCE`). `TerminalSession::is_tmux_backed()`
  treats both as attachable.
- **TerminalSession persistence has SQLite and Postgres mirrors.** Rows map in
  `lfdb/sqlite.rs` and `lfdb/postgres.rs`; both use explicit column
  lists. New columns still require schema/mirror work, but new `source` values
  do not.
- **Attach contract remains the shared path.** `POST
  /v0/terminal-sessions/{id}/attach` returns `TerminalConnectionInfoDto
  {session_name,host,cwd,status}`. Swift terminal panes should call
  `RepoState.attachTerminalSession(_:)` and attach Ghostty to the returned tmux
  session; do not recreate a parallel client-side tmux name.
- **Palette create path now exists.** `POST /v0/terminal-sessions` takes
  `{wave_id, flow, worktree, agent}` and returns `{session, connection}`. The
  executor builds `lf <flow> --no-direction ... -w <wave> -m <agent>`, stores
  `source="palette"`, and launches tmux via lfd.
- **Palette lifecycle completion is exit-file based.** Wave-run tmux sessions
  still exit the shell and complete after `tmux has-session` goes false.
  Palette sessions write `.lf/tmp/terminal-sessions/<id>.exit`, mark the row
  terminal, remove the temporary exit file, then keep the tmux pane alive via
  `exec "${SHELL:-/bin/zsh}"` so the pane stays attachable. Startup reconcile
  re-arms palette exit-file watchers when tmux is still live and completes rows
  whose tmux session died while lfd was down.
- **Multiplexer pane binding stores lfd session ids.** `PaneConfig` uses
  `terminalSessionId`; default terminal panes no longer synthesize
  `lf-<waveId>-<paneId>`. A pane without a session id shows an empty-state until
  a palette launch binds it.
- **Terminal pane config is now only durable identity, not launch intent.**
  `PaneConfig.launchCommand` and multiplexer config normalization were removed
  on 2026-05-19; palette launches create lfd terminal sessions directly and
  then bind `terminalSessionId`.
- **Palette sessions may be terminal in lfd while still attachable in tmux.**
  A succeeded palette row means the flow exited and the pane dropped into a
  shell, not necessarily that the tmux session is gone. Swift terminal panes
  should still attach by session id instead of hiding terminal-status sessions.
- **DTO fixtures now cover terminal sessions.** Fixtures in
  `tests/fixtures/dto/terminal_session.json` and
  `create_terminal_session_request.json` are asserted in Rust, Swift, and
  Python fixture tests.
- **lfd readiness probes are split by route prefix.** `/health` is an
  unauthenticated root route, while most API calls live under `/v0`. Validation
  scripts that wait for local lfd should probe `http://127.0.0.1:2486/health`
  instead of `/v0/status` (`/status` is root and auth-protected).

## Wave-agent session lifecycle (verified 2026-07-05, ghost-reclaim fix)

- **A live server is the ONLY hard block on launching a wave — not a tmux
  session, not a `.wave-endpoint` pointer file.** Both artifacts outlive a
  crash: a SIGKILL leaves the pointer behind, and a dead `lf wave` leaves its
  tmux session behind. The server's own boot floor is the real one-brain
  guarantee; the launcher just clears the name for it.
- **A tmux session that exists with no live endpoint is a GHOST, not a starting
  wave — reclaim (kill) it, don't refuse forever.** This was the Start-button
  bug: a crashed `lf wave` left `lf-<repo>-<wave>` behind and the old
  `launchBlockReason` guard blocked Start permanently. `launchAction` now
  returns `.blocked` / `.reclaim` / `.launch`
  (`MacLocalWaveAgentLauncher.swift`); a live PROBED endpoint blocks, a ghost
  session is reclaimed, otherwise launch.
- **Grace-probe before declaring a ghost.** `awaitLiveEndpoint(attempts:)`
  polls the live endpoint up to N times, 1s apart, to cover the mid-boot window
  where `lf wave` is up but hasn't published `.wave-endpoint` yet. Use 3 probes
  when a session already exists (don't kill a wave that's still coming up), 1
  when there's no session.
- **CLI counterpart is `lf op reset-waves`** (systems' `lf op` surface): kills
  every `lf-*` tmux session and clears stale `wave/*/.wave-endpoint` pointers
  under the main repo — the operator fresh-start button. Same ghost invariant,
  applied in bulk. tmux SIGHUPs the session's process group, so killing the
  `lf-` sessions takes the wave minds with them; lfd reconciles its registry on
  next boot.
- **Wave state lives at the wave's ORIGIN repo (`WaveOrigin.resolve`), not the
  worktree.** Resolve the origin once and feed that one path to session-name,
  endpoint read, and launch — a worktree `repoPath` must not fork these.

## Patterns (verified 2026-06-30, remote TLS connection)

- **Concerto reaches a native remote `lfd` over HTTPS via Tailscale, not TLS in
  `lfd`.** `deploy/tailscale-lfd-host.sh` keeps native `lfd` on `127.0.0.1` and
  runs `tailscale serve` as the HTTPS ingress with a real `*.ts.net` cert.
  `deploy/native-lfd-host.sh` owns the launchd lifecycle. The wrapper
  deliberately does not expose native `serve` — that stays an internal launchd
  entrypoint. Keep TLS termination outside `lfd`; don't add TLS serving to the
  daemon.
- **Remote bearer token is read fresh from `~/.lf/concerto.yaml` per request.**
  `RemoteConnectionConfig` (`swift/LoopflowCore/Config/ConcertoConfig.swift`)
  now carries an optional `token`. `ConnectionStore.token(for:)` prefers the
  config token over the static/Keychain token, but only when config host+port
  match the active connection — a token in one profile can't leak to another.
  This makes rotation immediate without re-pasting into Settings. `configLoader`
  is `@escaping` and held on the store so the read stays live.
- **CA-trusted certs (incl. `*.ts.net`) use system trust, not pinning.**
  `CertificatePinningDelegate` skips pinning for CA-trusted chains, avoiding
  false positives when Tailscale renews certs.
- **Dev builds use bundle id `com.loopflow.concerto.dev`.** `scripts/concerto-dev.py`
  rewrites the assembled `Concerto Dev.app` Info.plist (source plist unchanged),
  so worktree dev runs don't overwrite installed-app remote settings.
- **macOS UI-test mode skips bundled daemons and remote subscriptions.** Guard
  in `PortfolioWindow.swift` / `ConnectionStore`; UI tests must not touch a live
  remote host.

## Not yet built (remote connection)

- No multi-profile Concerto config — the schema is one remote `connection` plus
  optional container settings. Multi-host profiles are speculative; not
  compelling with a single Mac mini host today.
- No live-tailnet integration test in CI — coverage is script syntax/help
  behavior plus `ConcertoConfigTests`/`ConnectionStoreTests`. A real
  `tailscale serve` round-trip is untested.
- No bundled TLS inside `lfd` — this is a rejected alternative, not a gap.

## Preferences

- For this wave, prefer lfd-owned terminal sessions over Swift-owned tmux.
  Client-side tmux remains only for the older workspace user-shell escape path
  until a user-shell create endpoint exists.

## Learnings

- Kickoff line numbers drift fast — re-verify before citing in a design. The
  embedded-terminal kickoff was directionally right but off by 10-50 lines and
  pointed at a generic `store/mod.rs` when persistence is split across concrete
  store backends.
- The high-value review-design move here was catching invented fields that
  duplicate existing ones, not re-litigating the approach. The approach held.
- `cargo test -p loopflow dto_fixtures` filters by test name, not integration
  test file; use `cargo test -p loopflow --test dto_fixtures` to run that file.
- Headless loopflow runs set `LF_RUN_ID`; Rust tests that assert generated
  journal ids or branch-derived ingest claims must clear that environment
  explicitly or full `cargo test -p loopflow` fails only under agent-driven
  validation.
