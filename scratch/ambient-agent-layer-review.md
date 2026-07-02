# Ambient Agent Layer — how to evaluate

Forward-looking follow-ups (goal handoff, lfd wire switch, shared-path risk) are
folded into `wave/workflows/3-unify-operate-prompt.md`. What's below is the
reviewer recipe.

## Validation

Gate checks that passed on this branch:

```bash
cargo fmt --all -- --check
cargo test -p loopflow
cargo build
cargo clippy --all-targets -- -D warnings
cargo test --all
git diff --check
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
