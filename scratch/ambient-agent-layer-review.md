# Ambient Agent Layer Review

## What was implemented

Loopflow's default prompt layer is leaner. The always-injected `<lf:rlm>` block
and bundled fallback voice prompt are gone, while repo/user voice files still
render for interactive surfaces. A new opt-in `<lf:operate>` section is sourced
from `rust/loopflow/src/engine/builtins/OPERATE.md` and enabled with
`lf --operate`.

The `--operate` flag threads through CLI prompt launch, canonical launch prep,
prompt rendering, reproducible command output, and vendor skill seed handoff.
Session-launched prompts keep `operate: false`.

## Key choices

- `OPERATE.md` is the single builtin source instead of an inline const.
- `format_system_sections` now owns system-safe prompt rendering, and skill
  handoffs reuse it so surface, voice, and operate sections stay in sync.
- Voice remains a hook for `.lf/voice.md` and `~/.lf/voice.md`, not a shipped
  default prompt. It still only appears on interactive surfaces.
- `--operate` stays local to launch preparation. It does not cross the lfd DTO
  boundary, so no wire fixture churn.
- The committed scratch design artifact was removed because CI rejects anything
  under `scratch/` except `.gitkeep`.

## How it fits together

`lf` parses `--operate` into `Cli`, passes it to `LaunchPromptInput`, and sets
`PromptComponents.operate` after context gathering. Prompt assembly renders
`<lf:operate>` from `OPERATE_DOC` through `format_system_sections`; Claude system
prompts and skill launch seeds both use that same path.

## Risks and bottlenecks

Prompt assembly now relies more directly on `format_system_sections` for token
budgeting and skill handoffs. The main risk is a future caller manually
reconstructing system sections and drifting from this shared path.

`lf-prompt` does not expose `--operate`; parity goldens cover the default prompt
shape, while Rust unit tests cover operate injection.

No runtime performance concern was found. This change removes default prompt
content; affected goldens no longer start with the old 152-line `<lf:rlm>` block.

## What's not included

- `loopflow.goal` still has its own operating prompt handoff; the design leaves
  that for later.
- lfd sessions do not accept an operate flag.
- No bundled fallback voice prompt remains. Repos/users opt in with voice files.

## Validation

- `cargo fmt --all -- --check` passed.
- `cargo test -p loopflow` passed.
- `cargo build` passed.
- `cargo clippy --all-targets -- -D warnings` passed.
- `cargo test --all` passed.
- `git diff --check` passed.
- `rg -n "RLM_DOC|VOICE_DOC|lf:rlm" rust tests STYLE.md README.md docs scratch` returned no matches before handoff files were written.
- CI `scratch-clear` logic passed on a clean working tree before this gate
  handoff file was written.
- `cargo run -q -p loopflow --bin lf -- --help` shows `--operate`.
