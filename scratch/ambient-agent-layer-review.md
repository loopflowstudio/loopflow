# Ambient Agent Layer — review guide

Forward-looking follow-ups (goal handoff, lfd wire switch, shared-path risk) are
folded into `wave/workflows/3-unify-operate-prompt.md`. What's below is the
reviewer recipe.

## What was implemented

Default prompt assembly no longer injects the old bundled `<lf:rlm>` block or a
builtin fallback voice prompt. Loopflow operating guidance now lives in
`OPERATE.md` and appears only when `--operate` is set. The flag is available on
`lf`, `lf-prompt`, post-step argument reordering, reproducible command output,
launch prep, and vendor skill seeds.

## Key choices

- Keep voice guidance as repo/user preference only: `.lf/voice.md` and
  `~/.lf/voice.md` still work for interactive surfaces, but there is no bundled
  default voice.
- Reuse `format_system_sections` for full prompts, Claude system prompts, and
  vendor skill seeds so operate, voice, and surface rendering have one owner.
- Leave lfd sessions at `operate: false`; adding a wire field would need DTO
  fixture work across Rust, Python, and Swift, which is outside this branch.

## How it fits together

`GatherContextOpts` carries the local `operate` boolean into `PromptComponents`.
`format_system_sections` renders `<lf:operate>`, `<lf:voice>`, and the surface
instructions; launch prep splits those same sections into Claude system prompt
content while repo docs, diffs, wave context, clipboard, and tasks stay in the
task prompt. Skill handoff seeds call the same renderer instead of reconstructing
surface/voice/operate sections.

## Risks and bottlenecks

The main drift risk is future callers assembling system-safe sections by hand
instead of routing through `format_system_sections`. `lfd` cannot request
operate-mode prompts yet, so wave-launched sessions that need this guidance must
wait for the later wire-level design. Prompt size improves by deleting the
152-line default RLM block from every golden prompt; `--operate` adds that
guidance back only for runs that need loopflow operations.

## What's not included

- The `loopflow.goal` handoff to read `OPERATE.md`.
- A wire-level `operate` switch for lfd sessions.
- Any compatibility shim for the deleted `RLM.md` or `VOICE.md` builtins.

## Validation

Gate checks that passed on this branch:

```bash
cargo fmt --all -- --check
cargo test -p loopflow
cargo build
cargo clippy --all-targets -- -D warnings
cargo test --all
git diff --check
git diff --name-status main...HEAD -- scratch   # expect empty
```

Prompt-shape checks — default lean prompt injects none of the operating blocks:

```bash
cargo run -q -p loopflow --bin lf-prompt -- --repo tests/fixtures/prompt/basic \
  --surface headless --lfdocs false --diff-files false --diff false --clipboard false \
  | rg -n '<lf:operate>|<lf:voice>|<lf:rlm>' || true   # expect no matches
```

`--operate` injects the operate block:

```bash
cargo run -q -p loopflow --bin lf-prompt -- --repo tests/fixtures/prompt/basic \
  --surface headless --operate --lfdocs false --diff-files false --diff false --clipboard false \
  | rg -n '<lf:operate>|lf op commit|</lf:operate>'    # expect the operate block
```

No stale references to the deleted blocks, and the post-step flag is recognized:

```bash
rg -n "RLM_DOC|VOICE_DOC|lf:rlm" rust tests STYLE.md README.md docs   # expect empty
cargo run -q -p loopflow --bin lf -- gate --operate --help | rg -- '--operate'
```

`scratch/` handoff files are intentionally local-only. The branch no longer
commits them because CI's `scratch-clear` job allows only `scratch/.gitkeep`.
