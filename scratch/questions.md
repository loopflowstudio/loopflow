# Decisions — wave viewer draft, next pass (2026-07-07)

Jack resolved the three review findings:

## 1. RunStatus: bias to `lf`

Align `RunStatus` to the vocabulary `lf` actually emits — its lowercase tokens
(`running`, `ok`, `waiting`, `failed`, `pending`, …), not the lfd int enum. Drop
the invented `cancelled`. An unknown status must be **loud** (surface it), never a
silent `?? .pending`. When `lf` and lfd disagree, `lf` wins.

## 2. Runs UI: keep it simple

Runs is **not** the most important window. Do not build the origin-grouped
chart/history — a plain, minimal runs list is enough. Don't grow `lf runs` for
origin data now. The plan (objective + projects) is the centerpiece; runs are
secondary.

## 3. Kill the HTTP-to-lfd path in this PR

`LocalWaveService` — and **any** HTTP-to-lfd-as-API code (`WaveServiceProtocol`,
`httpBaseURL` data reads) — does **not** survive this PR. Converge every data
read on `RegistryQuery` (subprocess `lf … --json`, daemon-less). `RegistryQuery`
already covers `waves()`, `status()`, `recentRuns()`; reroute the remaining
~22 consumers (RepoState, PortfolioRepoState, SessionState, OutputBuffer,
WavesView, auth/connection) onto it and delete the HTTP service + its tests.
Keep one implementation.

## Diagnosis (filed under the Performance project — not pursued yet)

**Slow repo-load = reads gated on the bundled daemon.** `WavesView.syncRepoStates`
early-returns while `SharedDaemon.currentConnection == nil`, and
`prepareConnectionIfNeeded` `await`s `SharedDaemon.manager.start()` first — so the
wave list waits on lfd booting even though `RegistryQuery`/`lf ls` is daemon-less
(it's already wired: `RegistryQueryLocal.shared`, WavesView:562). Fix when
pursued: paint the list from `lf` immediately; start the bundled daemon in the
background for pubsub only, never as a barrier before the first read. This is the
"reads never block on lfd" KR's first instance.

## Perf audit — cheap wins, ranked (subagent, 2026-07-07; parked under Performance)

The 5s poll re-pays the full cold-start cost every cycle, multiplied across repos:

1. **`lf ls --json` fan-out** (S/M·H) — machine-wide read called once *per repo*
   (`RegistryQuery.swift:40`, `WavesView.swift:587`, `PortfolioRepoState.swift:79`);
   M identical subprocesses per refresh. Fix: one `allWaves()` per poll, distribute.
2. **Capability probe per query** (S·M) — `lf help wave` spawned before every real
   call (`MacLocalWaveAgentLauncher.swift:212`); 2 spawns per read. Fix: memoize.
3. **Daemon gate on first paint** (S/M·H) — `WavesView.swift:148/582`; drop the
   `SharedDaemon.currentConnection == nil` guard, boot lfd concurrently
   (pubsub-only). The known archetype, confirmed.
4. **`WavePlanParser.parse` in `body`** (M·H) — sync disk I/O (GOAL.md + every
   project file) re-runs on main thread per re-render (`WavesView.swift:60-90`,
   `PortfolioRepoState.swift:90-100`). Fix: parse off-main once per refresh, cache.
5. **tmux spawn storm** (S/M·M) — `tmux has-session` per authored wave per repo
   every 5s (`WavesView.swift:508-545`). Fix: one `list-sessions` → Set lookup.
6. **Unconditional 5s poll** (S·M) — mostly subsumed by 1/2/4/5.

Non-findings: `~/src` scan already cheap (no per-dir git spawn); `WaveOrigin`
memoized; runs ledger has no renderer yet. Minor: cold-start double-refresh via
`onChange(repos)` after `registerInitialRepoIfNeeded`.

## Minor (fold in)
- `RunSnapshot.toRun` `area: "."` placeholder — drop the field from `Run` if the
  snapshot can't supply it.
- `createdAt: … ?? Date()` — avoid the nondeterministic default.

## Perf audit — cheap wins

Ranked pass implemented in this branch:

1. One machine-wide registry read per poll: `RegistryQuery.allWaves()` shells
   `lf ls --json` once, then `WavesView.syncRepoStates()` distributes that slice
   to each `PortfolioRepoState` instead of asking each repo state to spawn its
   own query. Anchors: `swift/LoopflowCore/Services/RegistryQuery.swift:37`,
   `swift/Concerto/Platform/macOS/Views/WavesView.swift:600`,
   `swift/Concerto/State/PortfolioRepoState.swift:91`.
2. `lf` binary resolution is memoized behind a locked cache, so polling no longer
   repeats the `lf help wave` capability probe for the same candidate set.
   Anchors: `swift/Concerto/Platform/macOS/MacLocalWaveAgentLauncher.swift:30`,
   `swift/Concerto/Platform/macOS/MacLocalWaveAgentLauncher.swift:176`,
   `swift/Concerto/Platform/macOS/MacLocalWaveAgentLauncher.swift:313`.
3. First paint is not gated on the bundled daemon: `WavesView` starts lfd
   concurrently, paints from local `lf` registry data, then awaits daemon startup
   only before auth/pubsub work. Anchors:
   `swift/Concerto/Platform/macOS/Views/WavesView.swift:150`,
   `swift/Concerto/Platform/macOS/Views/WavesView.swift:169`,
   `swift/Concerto/Platform/macOS/Views/WavesView.swift:587`.
4. `WavePlanParser.parse` moved off render/body paths for the wave list. The view
   builds a per-refresh plan cache on a detached task and row construction reads
   from that cache. Anchors:
   `swift/Concerto/Platform/macOS/Views/WavesView.swift:626`,
   `swift/Concerto/Platform/macOS/Views/WavesView.swift:641`,
   `swift/Concerto/State/PortfolioRepoState.swift:96`.
5. Authored-wave status uses one `tmux list-sessions -F '#S'` snapshot per
   refresh, then does `Set` lookups, instead of spawning `tmux has-session` per
   wave. Anchors:
   `swift/Concerto/Platform/macOS/MacLocalWaveAgentLauncher.swift:219`,
   `swift/Concerto/Platform/macOS/Views/WavesView.swift:523`,
   `swift/Concerto/Platform/macOS/Views/WavesView.swift:558`.

Out of scope by instruction: perf monitoring, budgets, metrics infra, and
regression harnesses.

## Product gap found killing the architecture wave (2026-07-07)

No `lf wave stop <name>` exists. Choices were raw `tmux kill-session` (used) or
`lf op reset-waves` (kills EVERY lf-* session — too broad). A wave killed
out-of-band leaves its registry row `status: running` forever; only the probed
LIVE column tells the truth. Wants: a single-wave stop verb that kills the
session, clears `.wave-endpoint`, and settles the registry status. Belongs to
goals/systems waves, not this PR — filed here so it isn't lost (Linear auth
expired).

## Compress pass — reductions taken + deferred (2026-07-07)

Took (safe, all 303 swift-package tests green):
- Deleted `WaveService.checkAvailability()` — returned `true`, zero callers, not
  in any protocol.
- Deleted the always-empty `supportedHarnesses` throughout: `WaveFlowsResult`
  field + init param, `listFlowsAndDirections` construction, and the
  `RepoState.supportedHarnesses` published property with its four writes. It was
  written in four places and read nowhere.

Deferred (real duplication, but blocked — a human should decide):
1. `WaveService` is a ~600-line retired-lfd-HTTP facade whose ~25 action methods
   all `throw unsupported(...)`. They aren't dead — RepoState/SessionState/
   AuthProviderStore still call them behind live UI actions (stop, delete, land,
   next, addTrigger, combinePRs, session create/attach/cancel). Collapsing the
   facade means deleting those call paths and the UI actions that surface the
   error — a behavior change tied to the active session-lifecycle and
   wave-conducting projects, not a compress edit.
2. `WaveService.parse*FromJSON` (the dict-based parser, ~260 lines) is a SECOND
   wire mirror of the same types `RegistryQuery` now decodes via Codable — the
   drift hazard CLAUDE.md's DTO rule warns about. It's not deletable in isolation:
   `parseSessionFromJSON` backs the mandated `session.json` DTO fixture test, and
   `parseWaveFromJSON`/`parseAttentionFromJSON` back the wire-contract tests
   (ContractTests, WaveTests, AttentionStoreTests). Consolidating means migrating
   those contract/fixture tests onto RegistryQuery's Codable path — a real
   refactor across four untouched test files, out of scope for a compress pass.
3. Duplicate fractional-ISO8601 date parsing exists in both `WaveService.parseDate`
   and `RegistryQuery.RegistrySnapshotDate.parse` (identical logic). Consolidation
   is coupled to (2) — the WaveService copy dies when the dict parser does.
