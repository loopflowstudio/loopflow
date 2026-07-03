# A2 — Wave repo→repos split: implementation contract

Build-green, 7 steps. `Wave.repo: String` → `repos: Vec<RepoWork>`; identity stays
on Wave; execution (repo, worktree, branch, status, iteration, activeRun, commits,
diffStat, openPRCount, pr) moves into RepoWork; `Wave.status`/`iteration` roll up.

**Confirmed decisions**
- Rollup: `Paused` if wave-level `paused`; else any `running`→running; else any
  `failed`→failed; else any `waiting`→waiting; else idle. `iteration` = max over repos.
- **Global wave-name uniqueness** (drop `UNIQUE(name, repo)`; reconcile
  `get_wave_by_name` / `wave_name_exists` to name-only).
- `commits`/`diff_stat`/`open_pr_count`/`active_run` are DERIVED at DTO-build
  time (git + live-PR), never persisted — DTO `RepoWork` only, not `wave_repos`.
- Temporary bridge `Wave::primary_repo()` exists only in Rust source during the
  migration; deleted in Step 7. No persisted compat shim. DB `repo` column dropped
  in the final migration.

**Baseline refs**
- Core `Wave`: `types/wave.rs:229` (`repo` 233, `status` 242, `iteration` 243,
  `cycle_start_iteration` 248, `new` 257, accessors 285/313/317).
- `WaveRun`/`WaveRunSnapshot`: `types/wave.rs:343-390` (per-repo execution already here).
- Store schema: `store/migrations/001_initial.sql` (`repo TEXT NOT NULL`,
  `UNIQUE(name,repo)`, `idx_waves_repo`); `map_wave_row` `store/rows.rs:103`;
  SQL templates `store/catalog.rs` (ListWaves 171, ListWavesByRepo 176, UpsertWave
  181, GetWaveById 186, GetWaveByName 191, `list_waves_query` 558); sqlite writer
  `store/sqlite.rs:207`; postgres mirror `store/postgres.rs:225`; migration runner
  `store/migrations.rs:17` (main now owns `040_session_use` / `041_session_parent`;
  the wave-repos migration is `042_wave_repos`, and Step 7's drop migration is `043`).
- Client DTO: assembled `WaveDto` `http/dto.rs:81` via `build_wave_dto`
  `http/routes/mod.rs:75` (flattened execution fields 139-162; `infer_wave_git_state`
  mod.rs:221 + live_snapshot). NOT `types::wave::Wave`.
- Python mirror `python/loopflow/models.py:79` (has pydantic defaults that already
  violate the no-defaults rule — new `repos` must have NO default).
- Swift mirror `swift/LoopflowCore/Models/Wave.swift:196`; manual decode
  `LocalWaveService.swift:979 parseWaveFromJSON`; presentation `WaveViewModel.swift`
  (55/56/240 repoint to selected RepoWork).
- Fixtures: golden `tests/fixtures/wave.json` (Rust `tests/dto_fixtures.rs`, Python
  `test_contract.py:17`, Swift `ContractTests.swift:28`). `tests/fixtures/dto/` has
  no wave fixture today.

## Steps (checkpoint = build/tests green)

- [x] **1 — Additive: RepoWork type + `wave_repos` table + store methods.** DONE, green.
  Note: `replace_wave_repos` is a `Store` convenience wrapper; trait carries
  `list_wave_repos`/`upsert_wave_repo`/`delete_wave_repos` (mirrors `wave_crons`). Add
  `struct RepoWork { repo, worktree, branch, status, iteration, cycle_start_iteration }`
  to `types/wave.rs` (don't touch `Wave`). Migration `042_wave_repos.sql`
  (create + backfill from `waves`), register in `migrations.rs`. Store methods
  `list_wave_repos` / `upsert_wave_repo` / `replace_wave_repos` in `store/mod.rs` +
  sqlite + postgres, new `catalog::Query` entries, `map_wave_repo_row` in `rows.rs`.
  Old `waves` columns intact. Nothing reads `wave_repos` yet.

- [x] **2 — `Wave.repos` field + bridge.** DONE, green. Add `pub repos: Vec<RepoWork>`; keep
  `repo/status/iteration/cycle_start_iteration`. Add `primary_repo()`. Store loads
  `repos` (second query, like crons); `upsert_wave` also `replace_wave_repos` from
  flat fields. Update `Wave::new` + every `Wave{…}` literal (blast radius — compiler
  guides) to set `repos: vec![RepoWork{repo,…}]`. DTO still reads flat fields.

- [ ] **3 — Executor onto RepoWork.** Rewrite worktree + run-dispatch sites
  (helpers.rs 52/60/129/147, local.rs 144, loop_ticker.rs 81, wave/mod.rs 1403/1412,
  docker/mod.rs 651, queue.rs 742) to resolve repo from a RepoWork; iterate
  `for rw in &wave.repos` where work is created, else `primary_repo()`.

- [ ] **4 — status/iteration → RepoWork; Wave rollups.** helpers.rs:88-97 updates
  the RepoWork row not `wave.status`. `Wave::status()`/`iteration()` become rollups.
  Repoint readers (loop_ticker 93/96 max-iter, ResetStaleActiveWaves, build_wave_dto).

- [ ] **5 — Nest wire DTO.** `http/dto.rs`: `RepoWorkDto` (skip_serializing_if ok,
  NO serde default); remove flattened fields from `WaveDto`; `WaveDto.status` =
  rollup, `repos: Vec<RepoWorkDto>`. `build_wave_dto` loops per repo (git infer +
  live-PR per repo). Update in-tree consumers/tests (waves.rs, ws.rs).

- [ ] **6 — Clients + fixtures together.** Rewrite `tests/fixtures/wave.json`
  (nested `repos:[…]`). Python `class RepoWork` + `Wave.repos` (no defaults), drop
  flattened, fix `test_contract.py`. Swift `struct RepoWork` + `repos`, fix
  `parseWaveFromJSON`, repoint `WaveViewModel`, fix `ContractTests`. Add Rust wave
  assertion in `dto_fixtures.rs`. `cargo test` + `pytest` + `swift test` together.

- [ ] **7 — Remove bridge + drop legacy columns.** Delete `primary_repo()` + flat
  `Wave.repo/status/iteration/cycle_start_iteration`. Migration `043` drops
  `waves.repo/status/iteration/cycle_start_iteration`, `idx_waves_repo`, adjusts
  `UNIQUE(name,repo)` → global name uniqueness. Update catalog templates + param
  arrays + `map_wave_row`. Reconcile repo-filter semantics (waves.rs:180,
  terminal_sessions.rs:90, attention.rs:217, repos.rs:228) to "wave has a RepoWork
  matching repo". No `primary_repo`, no `repo` column left.

**Riskiest coupling:** removing `Wave.repo` is an atomic compile break across ~35
struct literals + every `.repo()` caller. Staged via `primary_repo()` in Step 2,
deleted Step 7.
