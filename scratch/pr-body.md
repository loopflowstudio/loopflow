## Try it!

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all
uv run python tests/goldens/update_goldens.py
tests/e2e/test_smoke.sh
swift test --package-path swift
```

Prompt behavior to inspect:

```bash
cargo run --bin lf-prompt -- --repo . --step gate --surface headless --diff-files false --diff false
cargo run --bin lf-prompt -- --repo . --step gate --surface headless --docs README.md,docs/ --diff-files false --diff false
```

The first command includes ambient scratch/loopflow context without root README docs. The second adds explicit docs without removing ambient context.

## Intent

Simplify prompt context generation around what users ask for directly: ambient working state is always present, docs are explicitly requested with `--docs`, and token accounting reports size without silently trimming context.

## Assumptions

Old prompt config keys are internal and can break. `lfdocs`, `area`, and context budgets are removed instead of migrated at runtime. Large scratch or wave memory should be fixed at the source rather than hidden by trimming.

## Key decisions

- Replaced `lfdocs` and `--area` with additive `docs` config and `--docs`.
- Kept scratch and wave docs ambient, with wave memory loaded separately.
- Replaced budgeted context mutation with `measure_context()`.
- Renamed prompt/session source labels from `repo_doc` / `area` to `docs`.
- Restored `--diff-files` branch-file behavior so enabling it with no explicit file list includes full changed file bodies.

## Not included

No compatibility shim for removed flags or config keys. Browser-facing website tests and generated Xcode project UI tests were not run in this headless gate because the run context said no rendering environment.
