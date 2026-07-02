## Try it!

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all
uv run python tests/goldens/update_goldens.py
uv run pytest python/tests/
swift test --package-path swift
tests/e2e/test_smoke.sh
uv run pytest tests/e2e/test_api_smoke.py tests/e2e/test_concurrent_clients.py -v
```

Prompt behavior to inspect:

```bash
cargo run --bin lf-prompt -- --repo . --step gate --surface headless --diff-files false --diff false
cargo run --bin lf-prompt -- --repo . --step gate --surface headless --docs README.md,swift/ --diff-files false --diff false
```

The first command includes loopflow and scratch context without root README docs.
The second adds explicit docs without removing ambient context.

## Intent

Simplify prompt context generation around direct user intent: ambient working
state is always present, docs are explicitly requested with `--docs`, diff and
clipboard are explicit switches, and token accounting reports size without
silently trimming gathered content.

## Assumptions

Old prompt config keys are internal and can break. `lfdocs`, `area`, and context
budgets are removed instead of migrated at runtime. Large scratch or wave memory
should be fixed at the source rather than hidden by trimming.

## Key decisions

- Replaced `lfdocs` and `--area` with additive `docs` config and `--docs`.
- Kept scratch and wave docs ambient, with wave memory loaded separately.
- Replaced budgeted context mutation with `measure_context()`.
- Renamed prompt/session source labels from `repo_doc` / `area` to `docs`.
- Kept `--diff-files` explicit; enabling it with no file list includes full
  changed file bodies.
- Applied gitignore/native-instruction filtering consistently to explicit docs
  directories and the `lf-prompt` inspection path.

## Not included

No compatibility shim for removed flags or config keys. Browser-facing website
tests and generated Xcode project UI tests were not run in this headless gate
because the run context said no rendering environment.
