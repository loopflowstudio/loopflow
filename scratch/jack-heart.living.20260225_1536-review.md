# Surface-adaptive prompts (Phase 03) — review

## What was implemented
- Added a first-class `Surface` enum (`headless`, `cli`, `concerto_mac`, `concerto_iphone`) to prompt assembly.
- Replaced `run_mode` in prompt assembly structs with `surface`:
  - `GatherContextOpts`
  - `PromptComponents`
  - `LaunchPromptInput`
- Updated prompt rendering so behavior instructions now come from `Surface::instructions()` (interaction + rendering guidance in one block).
- Wired surface selection by caller:
  - `lf` CLI always uses `Surface::Cli`
  - wave executor paths use `Surface::Headless`
  - session manager defaults to `Surface::ConcertoMac` and honors `SessionConfig.surface` override.
- Added `surface` to session config payloads and covered it with tests.
- Updated built-in LOOPFLOW docs from “Run Modes” to “Surfaces”.
- Updated golden fixtures and tests to use `surface` fields and new prompt text.

## Key choices
- **Single source of behavior truth:** `Surface::instructions()` centralizes the prompt behavior text, preventing mode/surface drift.
- **Safe unknown handling:** unknown surface values parse to `Headless` to avoid blocking on non-existent user input.
- **Client defaults at the edge:** session manager applies `ConcertoMac` default when no surface is provided; CLI and executor set explicit surfaces.
- **No DB migration in this phase:** legacy `run_mode` persists in agent execution/storage records; this change is scoped to prompt assembly/session prompt inputs.

## How it fits together
Prompt callers pass a `Surface` into `prepare_launch_prompt`, which forwards it through `gather_context` into `PromptComponents`. Formatting uses that surface to inject one behavior block into the prompt before wave/docs/diff sections. Session config adds optional `surface`, so Concerto can choose desktop vs mobile guidance per session.

## Risks and bottlenecks
- **Scope split risk:** runtime/storage still uses `run_mode`; prompt assembly now uses `surface`. This is intentional but means two concepts coexist until a follow-up migration.
- **Unknown session surface fallback:** typos/unknowns degrade to headless behavior (safe, but may reduce UX quality if clients send bad values).
- **Environment-dependent tests:** two Docker startup tests require `/var/run/docker.sock`; macOS UI `xcodebuild test` failed in this environment due UITest runner crash before bootstrap.

## What’s not included
- No token-budget/context-priority differences by surface.
- No CLI surface override flag for `lf run`.
- No database/API rename from `run_mode` to `surface` for persisted agent records.
- No Concerto client-side implementation changes in this repo (only server/session acceptance path).

## Validation run
- ✅ `cargo fmt --all -- --check`
- ✅ `cargo clippy --all-targets -- -D warnings`
- ✅ `cargo test --all -- --skip lfd::executor::docker::tests::docker_startup_lost_agent_does_not_flip_terminal_run_wave_status --skip lfd::executor::docker::tests::docker_startup_rehydrates_running_agents_and_cleans_orphans`
- ✅ `uv run pytest python/tests/`
- ✅ `swift test --package-path swift`
- ✅ `tests/e2e/test_smoke.sh`
- ⚠️ `cargo test --all` fails locally on 2 Docker-socket-dependent tests when Docker socket is unavailable.
- ⚠️ `cd swift && xcodegen generate && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO` failed due `ConcertoUITests-Runner` early bootstrap crash in this environment.
