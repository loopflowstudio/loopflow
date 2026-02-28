# 02: CI Speed

**Finish line:** The `rust-test` CI job completes in under 5 minutes on a warm cache. Python lint runs in CI.

## Scope

**Rust build caching.** Add `Swatinem/rust-cache` to `ci.yml` for the `rust-test`, `e2e-smoke`, and `docker-smoke` jobs. Currently 393 dependencies compile from scratch on every run.

**Docker layer caching.** In `docker/lfd/Dockerfile`, separate the dependency compilation step from the source copy: copy `Cargo.toml`/`Cargo.lock` first, run `cargo build --release`, then copy source. This lets Docker layer caching skip dep compilation on source-only changes.

**Python lint in CI.** Add a `ruff check` step to the `python-test` job. Ruff is already configured in `pyproject.toml` but never invoked in CI.

**Release caching.** Add caching to `release.yml` build matrix — four targets currently compile the full dep tree from scratch in parallel.

**Fix `pyproject.toml` version check.** Add a `pyproject.toml` version comparison to `auto-tag.yml` alongside the existing `Cargo.toml` check. Prevents publishing mismatched crate and Python package versions.

**Remove `--allow-dirty`.** Drop `--allow-dirty` from `cargo publish` in `release.yml`, or verify the workspace is clean before publishing.
