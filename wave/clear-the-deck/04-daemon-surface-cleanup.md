# 04: Daemon Surface Cleanup

**Finish line:** `lfd` management entrypoints, config overrides, and release-tag safety are covered by first-class parsing and focused tests instead of manual scanning and implicit gaps.

## Carried context

- `rust/loopflow/src/bin/lfd.rs` still dispatches `migrate`, `install`, `uninstall`, `start`, `stop`, and `status` by reading `std::env::args()` and scanning flags with `args.get(1)` / `has_flag()`.
- `output_log_retention_days` is part of persisted `lfd` config, but `apply_env_overrides()` still lacks an `LFD_OUTPUT_LOG_RETENTION_DAYS` path.
- `rust/loopflow/src/ops/release.rs` supports tagging a non-`HEAD` ref through `tag_and_push_ref(..., target_ref)`, but that path is only exercised indirectly via `release_run`.

## What to build

1. Replace the manual `bin/lfd.rs` subcommand parsing with Clap so daemon management and serve flags share one honest CLI surface.
2. Add the missing `LFD_OUTPUT_LOG_RETENTION_DAYS` override and cover it in config tests next to the existing env-override matrix.
3. Add focused release tests for `tag_and_push_ref` when `target_ref` points at a merged commit instead of `HEAD`.
4. Keep the Docker-only container-mode behavior intact while reshaping the parser and tests.

## Uncertainty

- The simplest Clap shape may still keep "serve" implicit when no subcommand is passed. Preserve that UX unless the parser becomes simpler by making serve explicit everywhere.
- The release-tag test can stay unit-level even if the helper remains private; use the smallest visibility change that proves the behavior.

## Done when

- `bin/lfd.rs` no longer switches management commands by manual string matching.
- `LFD_OUTPUT_LOG_RETENTION_DAYS` overrides config the same way neighboring settings do.
- Release tests prove tagging a non-`HEAD` ref succeeds or fails with the expected SHA checks.
