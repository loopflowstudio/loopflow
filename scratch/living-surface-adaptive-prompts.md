# Phase 03: Surface-adaptive prompts

## Status

Implemented on this branch. Prompt assembly now keys behavior on `Surface` instead of `run_mode`.

## Problem

Prompt behavior previously changed only by `run_mode` (`auto` vs `interactive`). That captured interaction pattern, but not rendering context. CLI, Concerto macOS, and Concerto iPhone all got the same interactive instructions even though output needs differ by surface.

## Final design

Use a single `Surface` enum as the environmental signal.

| Surface | Interactive? | Primary environment |
|---------|---------------|---------------------|
| `headless` | no | wave executor, CI/batch |
| `cli` | yes | `lf` in terminal |
| `concerto_mac` | yes | Concerto desktop UI |
| `concerto_iphone` | yes | Concerto mobile UI |

Key behavior:
- `Surface::is_interactive()` replaces string checks like `run_mode == "interactive"`.
- `Surface::instructions()` is the single source of behavioral text injected into prompts.
- Unknown serialized surfaces safely degrade to `headless`.

## Data flow

Surface now flows through prompt assembly end-to-end:
1. `lf` CLI sets `Surface::Cli`.
2. Wave executor sets `Surface::Headless`.
3. Session manager defaults to `Surface::ConcertoMac` and honors `SessionConfig.surface`.
4. `LaunchPromptInput`, `GatherContextOpts`, and `PromptComponents` carry `surface`.
5. Prompt formatting reads `surface` and injects the appropriate instruction block.

## Instruction behavior by surface

- `cli`: interactive conversation guidance only.
- `headless`: autonomous guidance + `scratch/questions.md` fallback + log-oriented output guidance.
- `concerto_mac`: interactive guidance + scannable desktop output guidance.
- `concerto_iphone`: interactive guidance + concise/mobile output guidance.

## Scope delivered

- Added `Surface` enum (`headless`, `cli`, `concerto_mac`, `concerto_iphone`) in prompt engine.
- Replaced prompt-assembly `run_mode` fields with `surface`.
- Updated built-in `LOOPFLOW.md` docs from “Run Modes” to “Surfaces”.
- Added optional `surface` in session config payloads.
- Updated tests/goldens to use `surface`.

## Explicitly not part of this phase

- DB/storage rename from `run_mode` to `surface` for agent execution records.
- Surface-specific token budgets or context trimming.
- `lf run --surface` override (`lf-prompt` has `--surface` for parity tests; `lf run` hardcodes `Surface::Cli`).

## Risks and follow-ups

- **Model split remains**: prompt assembly uses `surface`, execution/storage still persists `run_mode`.
- **Unknown session values**: degrade to headless safely, but may hide client typos.
- **Follow-up needed**: decide whether/when to migrate persisted run metadata from `run_mode` to `surface`.

## Validation snapshot

- ✅ `cargo fmt --all -- --check`
- ✅ `cargo clippy --all-targets -- -D warnings`
- ✅ `cargo test --all -- --skip lfd::executor::docker::tests::docker_startup_lost_agent_does_not_flip_terminal_run_wave_status --skip lfd::executor::docker::tests::docker_startup_rehydrates_running_agents_and_cleans_orphans`
- ✅ `uv run pytest python/tests/`
- ✅ `swift test --package-path swift`
- ✅ `tests/e2e/test_smoke.sh`
- ⚠️ Full `cargo test --all` depends on Docker socket for two tests.
- ⚠️ Concerto macOS UI `xcodebuild test` failed in this environment due UITest runner bootstrap crash.
