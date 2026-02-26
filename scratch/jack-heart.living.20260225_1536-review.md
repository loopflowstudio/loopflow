# Review: Surface-adaptive prompts (Phase 03)

## What was implemented

Replaced `run_mode` string matching (`"auto"` / `"interactive"`) throughout prompt assembly with a typed `Surface` enum. Each surface variant carries its own behavioral instructions tailored to the rendering environment.

**Surface enum** (`Headless`, `Cli`, `ConcertoMac`, `ConcertoIphone`) in `engine/prompt.rs`:
- `is_interactive()` replaces string comparisons
- `instructions()` returns per-surface guidance from external `.md` files
- `#[serde(other)]` on `Headless` gives safe degradation for unknown values
- `#[non_exhaustive]` allows future surface additions

**Surface instruction files** in `engine/builtins/surfaces/`:
- One `.md` file per surface, loaded via `include_str!`
- Consistent with how `LOOPFLOW_DOC` and `RLM_DOC` are bundled
- Instructions editable without touching Rust code

## Key choices

| Decision | Why | Alternative rejected |
|----------|-----|---------------------|
| Single enum, not trait | Surfaces are a closed set with simple dispatch; no polymorphism needed | Trait objects — unnecessary complexity |
| `Headless` as default | Conservative: if surface is unknown, behave autonomously | `Cli` — too interactive for batch/CI |
| `Infallible` `FromStr` | Unknown strings degrade safely; same behavior as `serde(other)` | Error on unknown — would break forward compat |
| External `.md` files | Instruction text is long; editing Rust strings is unpleasant | Inline strings — harder to maintain |
| `Option<Surface>` in `SessionConfig` | Type safety at API boundary; serde handles parsing | `Option<String>` — required `.parse()` boilerplate |

## How it fits together

Surface flows through three entry points:
1. **CLI** (`lf run`) → always `Surface::Cli`
2. **Wave executor** (`lfd`) → always `Surface::Headless`
3. **Session manager** (`lfd sessions`) → from `SessionConfig.surface`, defaulting to `ConcertoMac`

All three converge at `GatherContextOpts.surface` → `PromptComponents.surface` → `format_reference_sections()` which injects the surface instruction block.

## Risks and bottlenecks

- **Prompt/storage model split.** Prompt assembly uses `Surface`; execution records still persist `run_mode`. Two mental models coexist. Acceptable while the prompt-side abstraction stabilizes.
- **Silent CLI fallback.** `lf-prompt --surface typo` silently becomes `Headless` via `Infallible` `FromStr`. This is intentional for programmatic callers but could confuse CLI users. A future improvement could derive `clap::ValueEnum` for CLI validation while keeping `FromStr` infallible for serde/API use.
- **Session default is `ConcertoMac`.** Sessions without an explicit surface get macOS-specific rendering guidance. Correct for the current use case (sessions are a Concerto feature) but would need revisiting if sessions are used from other surfaces.

## What's not included

- DB/storage migration from `run_mode` to `surface`
- Surface-specific token budgets or context trimming
- CLI `--surface` override for `lf run` (only `lf-prompt` has it)
- `clap::ValueEnum` derive for CLI argument validation

## Gate polish applied

- Renamed `format_prompt_empty_components` test to `format_prompt_default_components_has_headless_surface` (name matched old assertion, not current behavior)
- Removed misleading "(default)" label from `cli` in `LOOPFLOW.md` and all golden files (`Headless` is the Rust default, `Cli` is the CLI default — the label was ambiguous)
- Added explanatory comment on `ConcertoMac` session default
- Moved surface instruction text from inline Rust strings to `builtins/surfaces/*.md` files
- Upgraded `SessionConfig.surface` from `Option<String>` to `Option<Surface>` (removes parse boilerplate)

## Validation

- `cargo fmt --all -- --check` — pass
- `cargo clippy --all-targets -- -D warnings` — pass
- `cargo test --all` — pass
- `uv run pytest python/tests/` — 51 passed
- `swift test --package-path swift` — 148 passed
- `tests/e2e/test_smoke.sh` — pass
