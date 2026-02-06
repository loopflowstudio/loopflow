# Simplification Opportunities

## Product intent
Loopflow wants waves to be the single, stable unit of orchestration: a wave has a name, configuration, and a clear status, and clients (CLI, Python, Concerto) should be able to read and act on it without translations or extra lookups. The v1 API is meant to be the canonical surface — clients should not have to reconstruct state or run extra commands to understand what a wave/run represents.

## Opportunity 1: Make wave status first-class
**Misalignment**: The API exposes `status` as a core wave attribute, but the stored model only has a `paused` boolean and derives status from active runs at read time. Clients translate back to `paused` when updating, creating a round trip that doesn't match the product's "status is primary" mental model.
**Symptom**: `build_wave_dto()` synthesizes status from `paused` + active run lookups, and `Client.update_wave()` turns `status == "paused"` into a `paused` payload. Multiple handlers re-fetch runs just to compute status/iteration. The executor updates `WaveRun.status` but never writes the wave's own status — so every read has to re-derive it.
**Realignment**: Persist `status` (idle/running/waiting/paused/failed/completed) and `iteration` directly on `Wave`. The executor already knows when runs start, fail, complete, or wait — it just needs to write that to the wave too. `paused` becomes a status value, not a separate boolean.
**Cascade**: Remove `paused` translation in Python/Swift clients, simplify `build_wave_dto()` (no run-derived status), eliminate the second query for iteration when no active run exists, reduce handler queries, and make status updates explicit and predictable.

## Opportunity 2: Snapshot run configuration at run creation
**Misalignment**: A wave run is meant to represent a specific iteration, but its DTO is populated by pulling flow/area/direction from the current wave and PR data from a live `gh` lookup. Run history changes when the wave changes, and API reads execute side effects.
**Symptom**: `wave_run_dto()` requires a `Wave` reference to fill in flow/repo/direction/area — if the wave is missing, these fields become empty strings. `pr_for_run()` spawns `gh pr view` via `spawn_blocking` on every read request. In `list_wave_runs`, `load_wave_map()` does N+1 queries: list repo waves, then fetch each missing wave individually. Every run DTO repeats the same wave metadata.
**Realignment**: Store a "run snapshot" on `WaveRun` (flow, repo, direction, area) when a run is created. Store PR metadata when it's first discovered (at PR creation or landing). Treat run records as immutable history.
**Cascade**: Remove `pr_for_run()` entirely (no more `gh` shell-outs in the read path). Remove the `wave: Option<&Wave>` parameter from `wave_run_dto()`. Eliminate `load_wave_map()` and its N+1 queries. Make list endpoints fast regardless of wave/run count. Keep run history stable even if wave config changes later.

## Opportunity 3: Make wave names a first-class lookup key
**Misalignment**: The product allows name-or-ID addressing everywhere, but the storage layer only supports ID lookup, forcing every handler to list and scan waves to resolve names.
**Symptom**: `resolve_wave_id()` calls `list_waves()` and does a linear scan on every request. This runs in 8 of 9 wave handlers. At scale, every `GET /v1/waves/engbot` loads all waves to find the one named "engbot".
**Realignment**: Add `find_wave_by_name(&str)` to the `RunStore` trait. In SQLite, this is a single `SELECT ... WHERE name = ?` with a unique index. In-memory stores can use a `HashMap<String, LfdId>`.
**Cascade**: `resolve_wave_id()` becomes two fast paths (try ID parse, then name lookup) instead of one fast + one slow. Eliminates repeated `list_waves()` calls from individual handlers.

## Opportunity 4: Shrink the Swift Wave model to match the API contract
**Misalignment**: The Swift `Wave` struct has 30+ fields, but the v1 API only sends ~12. The remaining fields (`isDirty`, `isRebasing`, `isMerging`, `hasDiff`, `aheadMain`, `behindMain`, `aheadRemote`, `behindRemote`, `staleness`, `recentSteps`, `prLimit`, `mergeMode`, `pid`, `lastMainSha`, `waitingReason`, `flowSteps`, `runStartedAt`) are parsed from JSON keys the API never sends, so they're always defaults.
**Symptom**: `parseWaveFromJSON` is 130 lines of manual JSON extraction, most of which reads keys that don't exist in the response and silently produces zero/false/nil values. The Swift `Wave` model carries git status fields, staleness tracking, merge mode, PR limits, and flow progress state — none of which come from the v1 API. This ghost data was shaped for an older API surface that enriched waves with worktree and git state server-side.
**Realignment**: Split `Wave` into the API model (what lfd sends: id, name, repo, flow, direction, area, status, iteration, active_run, created_at) and a view model that adds UI-only state (staleness, git status, display properties). Parse only what the API sends. If fields like `recentSteps` or `flowSteps` are needed, add them as proper API expansions first, then parse them.
**Cascade**: `parseWaveFromJSON` drops from 130 lines to ~20. The Wave model becomes a clear contract with the API instead of a grab-bag. View-only concerns like `statusIndicator`, `displayName`, and `lastActivityDescription` move to the view model where they belong. Adding new API fields becomes obvious — you add them to the API model, not hunt through 30 existing fields.

## Aligned areas
- The v1 API shape (Stripe-style list envelopes, expandables, structured errors) matches the product intent of a clean, consistent client surface.
- The Python client's module-level API mirrors the CLI and Concerto expectations well — one canonical surface for humans and automation.
- The Python client is already thin and pass-through. It reflects server complexity rather than adding its own. Once opportunities 1-2 are addressed server-side, the Python client simplifies automatically.
- The executor's step-by-step flow execution is clean and well-structured — it just needs to write wave-level status when it writes run-level status.
