# Context generation: explicit docs, ambient state, measurement only

**Shipped on this branch.** Design detail now lives in `engine/prompt.rs`,
`engine/launch.rs`, and `engine/builtins.rs`. This file keeps only the intent and
the acceptance checks a reviewer uses to confirm it.

## Intent

Drop `lfdocs` as a product concept and simplify the context engine around what
users actually mean:

- `scratch/` and (when a wave is in scope) `wave/<name>/` + `MEMORY.md` are
  ambient working state, always included.
- `--docs` is explicit additive prefetch — never removes ambient context.
- diff and clipboard are explicit switches.
- token counts are visibility, not control: context is **measured, not trimmed**.
  No prompt content silently disappears because a budget was exceeded.
- `OPERATE.md`/`<lf:operate>` became `LOOPFLOW.md`/`<lf:loopflow>`, default-on
  with a `--no-loopflow` opt-out.

Config migration is breaking by design (internal config, no shims):
`area: <dir>` → `docs: [<dir>]`; `lfdocs: true` → `docs: ['*.md']`;
`budgets:` context settings removed.

## Done when (acceptance checks)

- Bare `lf gate` includes `<lf:loopflow>`, `<lf:scratch>`, wave docs/memory when
  scoped, and the native agent doc. It does **not** include root `*.md` docs.
- `lf gate --docs README.md,swift/` adds those docs without removing scratch or
  wave context.
- `--docs` resolving over `MAX_EXPLICIT_DOC_FILES` (100) fails clearly instead of
  trimming.
- No `<lf:docs>` section; `--area`, `--lfdocs`, `--no-lfdocs`, `--operate` are gone.
- Context size output still reports source tokens, file counts, document entries,
  and total tokens; no source is silently trimmed after gathering.
- `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test -p loopflow`,
  and `uv run python tests/goldens/update_goldens.py` pass.
