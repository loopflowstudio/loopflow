# Ambient Agent Layer Review

## What was implemented

Loopflow's default prompt layer is leaner. The always-injected `<lf:rlm>` block
and bundled fallback voice prompt are gone, while repo/user voice files still
render for interactive surfaces. A new opt-in `<lf:operate>` section is sourced
from `rust/loopflow/src/engine/builtins/OPERATE.md` and enabled with
`lf --operate`.

The `--operate` flag threads through CLI prompt launch, canonical launch prep,
prompt rendering, reproducible command output, `lf-prompt`, and vendor skill
seed handoff. Session-launched prompts keep `operate: false`.

## Key choices

- `OPERATE.md` is the single builtin source instead of an inline const.
- `format_system_sections` now owns system-safe prompt rendering, and skill
  handoffs reuse it so surface, voice, and operate sections stay in sync.
- Voice remains a hook for `.lf/voice.md` and `~/.lf/voice.md`, not a shipped
  default prompt. It still only appears on interactive surfaces.
- `--operate` stays local to launch preparation. It does not cross the lfd DTO
  boundary, so no wire fixture churn.
- Scratch handoff files are local-only now. CI's `scratch-clear` job rejects
  committed `scratch/` contents, while `lf op land` can still consume the local
  PR copy from this worktree.

## How it fits together

`lf` parses `--operate` into `Cli`, passes it to `LaunchPromptInput`, then into
`GatherContextOpts` and `PromptComponents.operate`. Prompt assembly renders
`<lf:operate>` from `OPERATE_DOC` through `format_system_sections`; Claude
system prompts and skill launch seeds both use that same path.

## Risks and bottlenecks

Prompt assembly now relies more directly on `format_system_sections` for token
budgeting and skill handoffs. The main risk is a future caller manually
reconstructing system sections and drifting from this shared path.

`lf-prompt` exposes `--operate`, but existing parity goldens cover the default
lean prompt shape. Rust unit tests and direct prompt checks cover operate
injection.

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
- `cargo run -q -p loopflow --bin lf-prompt -- --repo tests/fixtures/prompt/basic --surface headless --lfdocs false --diff-files false --diff false --clipboard false | rg -n '<lf:operate>|<lf:voice>|<lf:rlm>' || true` returned no matches.
- `cargo run -q -p loopflow --bin lf-prompt -- --repo tests/fixtures/prompt/basic --surface headless --operate --lfdocs false --diff-files false --diff false --clipboard false | rg -n '<lf:operate>|lf op commit|</lf:operate>'` found the operate block.
- `cargo test -p loopflow reorder_args_operate_flag_after_step` passed.
- `rg -n "RLM_DOC|VOICE_DOC|lf:rlm" rust tests STYLE.md README.md docs` returned no matches.
- Tracked `scratch/` contents are down to `scratch/.gitkeep`; the gate handoff
  files remain untracked for `lf op land`.
- `cargo run -q -p loopflow --bin lf -- gate --operate --help | rg -- '--operate'` shows the post-step flag is recognized.
