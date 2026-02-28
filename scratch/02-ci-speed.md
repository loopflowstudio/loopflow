# 02: CI Speed

## Problem

CI is slow and incomplete. The `rust-test` job compiles 393 dependencies from scratch on every run—a clean build that should be cached. Release builds do the same across 4 parallel targets. Python linting (ruff) is configured but never runs in CI, so lint regressions slip through. The Docker image rebuild is also slower than necessary because source changes invalidate the dependency compilation layer.

Separately, version safety has gaps: `auto-tag.yml` now checks Cargo.toml against RELEASE_NOTES.md, but pyproject.toml is unchecked—a mismatched Python package version could ship. And `cargo publish --allow-dirty` masks workspace cleanliness issues.

This serves the infra wave goal: "CI gives feedback in minutes, not 15+."

## Approach

Six targeted changes across four workflow files and one Dockerfile. Each is independent and could ship separately, but they're small enough to land together.

### 1. Rust build caching in CI (`ci.yml`)

Add `Swatinem/rust-cache@v2` after `dtolnay/rust-toolchain@stable` in three jobs:

- **rust-test** — the biggest win. Full dep tree + clippy + tests.
- **e2e-smoke** — builds the full binary for E2E tests.
- **docker-smoke** — builds the loopflow crate for docker tests.

Each job gets its own cache key automatically (Swatinem hashes based on Cargo.lock, rustc version, and job name). No configuration needed beyond adding the action.

```yaml
- uses: Swatinem/rust-cache@v2
```

### 2. Docker layer caching (`docker/lfd/Dockerfile`)

Split the build into two phases so Docker's layer cache can skip dependency compilation on source-only changes:

1. Copy `Cargo.toml`, `Cargo.lock`, and each workspace member's `Cargo.toml`
2. Create stub `lib.rs` files so cargo can resolve the dependency graph
3. `cargo build --release` — compiles all 393 deps (cached unless Cargo.lock changes)
4. Remove stubs, copy real source
5. `cargo build --release` again — only recompiles loopflow crates

```dockerfile
FROM rust:1.88-bookworm AS builder
WORKDIR /build

# --- Dependency layer (cached unless Cargo.lock changes) ---
COPY Cargo.toml Cargo.lock ./
COPY rust/loopflow/Cargo.toml rust/loopflow/Cargo.toml
COPY rust/loopflow-test-support/Cargo.toml rust/loopflow-test-support/Cargo.toml
RUN mkdir -p rust/loopflow/src/bin rust/loopflow-test-support/src \
    && touch rust/loopflow/src/lib.rs \
    && echo "fn main() {}" > rust/loopflow/src/bin/lf.rs \
    && echo "fn main() {}" > rust/loopflow/src/bin/lfd.rs \
    && echo "fn main() {}" > rust/loopflow/src/bin/lf-prompt.rs \
    && touch rust/loopflow-test-support/src/lib.rs
RUN cargo build -p loopflow --release

# --- Source layer (only your code recompiles) ---
COPY rust ./rust
RUN cargo build -p loopflow --release
```

Note: loopflow has no `main.rs` — it declares `lib.rs` + three `[[bin]]` targets (`lf`, `lfd`, `lf-prompt`) with explicit paths under `src/bin/`. The stubs must match the actual crate structure or `cargo build` will fail looking for the declared binary sources.

The runtime stage (debian:bookworm-slim, Python client install, entrypoint) is unchanged — only the builder stage gets the dependency-layer split.

### 3. Python lint in CI (`ci.yml`)

Add a `ruff check` step to the `python-test` job, before pytest:

```yaml
- name: Run ruff lint check
  run: uv run ruff check
```

Ruff uses the existing `[tool.ruff]` config in `pyproject.toml` (line-length=100, rules E/F/W/I, target py310). Running without arguments uses the config's defaults for file discovery.

### 4. Release build caching (`release.yml`)

Add `Swatinem/rust-cache@v2` to the `build-native` job. Use `shared-key` to avoid cache fragmentation across the 4-target matrix:

```yaml
- uses: Swatinem/rust-cache@v2
  with:
    shared-key: release-${{ matrix.target }}
```

Each target gets its own cache (different compilation artifacts), but within a target, subsequent releases reuse the dep cache.

### 5. pyproject.toml version check (`auto-tag.yml`)

Add a version comparison for pyproject.toml alongside the existing Cargo.toml check:

```bash
pypi_version=$(sed -n 's/^version = "\(.*\)"/\1/p' pyproject.toml | head -1)
if [ "$pypi_version" != "$expected" ]; then
  echo "::error::Version mismatch: RELEASE_NOTES.md says $expected but pyproject.toml has $pypi_version"
  exit 1
fi
```

This catches the case where Cargo.toml is bumped but pyproject.toml is forgotten (or vice versa).

### 6. Remove `--allow-dirty` (`release.yml`)

Drop `--allow-dirty` from `cargo publish` (line 206). The GitHub Actions checkout gives a clean workspace. If the workspace ever isn't clean, that's a bug we want to know about—`--allow-dirty` silently masks it.

```yaml
# Before
run: cargo publish -p loopflow --allow-dirty
# After
run: cargo publish -p loopflow
```

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| `sccache` instead of `Swatinem/rust-cache` | Compiler-level cache, works across jobs | More setup, S3/GCS backend needed. Swatinem is simpler for GitHub Actions and covers our case. |
| `cargo-chef` for Docker layer caching | Purpose-built tool for this exact problem | Adds a build dependency. The stub-lib approach works for our simple workspace (2 members) without extra tooling. |
| Separate CI job for ruff | Parallel execution, clearer failure reporting | Overhead of another job (checkout, setup-uv) for a sub-second check. Adding it to python-test is simpler. |
| Cache Docker layers with `docker/build-push-action` | GitHub Actions native Docker caching | docker-smoke doesn't build Docker images—it runs `cargo test`. The Dockerfile improvement benefits local builds and any future CI Docker builds. |

## Key decisions

**Swatinem/rust-cache v2, no configuration.** The action auto-detects Cargo.lock, toolchain, and job context. No need for custom cache keys in CI—the defaults are correct. Release builds get `shared-key` because the matrix would otherwise fragment caches by OS.

**Stub sources, not cargo-chef.** The workspace has exactly 2 members. A stub approach is a few lines of Dockerfile. cargo-chef adds a build tool, a new stage, and complexity that doesn't pay off at this scale. Stubs must match the actual crate layout (`lib.rs` + `src/bin/*.rs` for loopflow, `lib.rs` for test-support).

**Ruff in python-test, not its own job.** Ruff runs in ~200ms. A separate job adds 30+ seconds of setup overhead. Failing fast in the same job is better.

**Drop --allow-dirty, don't add a workspace-clean check.** The checkout action guarantees a clean workspace. An explicit check would be redundant. If something breaks this invariant, the publish failure will be informative.

## Scope

- In scope: ci.yml caching, release.yml caching, Dockerfile layer split, ruff in CI, pyproject.toml version check, remove --allow-dirty
- Out of scope: macOS/Swift job caching (different ecosystem, separate sprint), Python dep caching (uv is already fast), Docker image CI builds (we test cargo, not Docker)

## Done when

1. `rust-test` CI job completes in under 5 minutes on a warm cache (currently ~10-15 min)
2. `uv run ruff check` runs in the `python-test` CI job
3. `auto-tag.yml` fails if pyproject.toml version doesn't match RELEASE_NOTES.md
4. `cargo publish` runs without `--allow-dirty`
5. `docker build -f docker/lfd/Dockerfile .` reuses the dependency layer when only Rust source changes
6. Release builds use cached dependencies

Wave goals advanced: "CI gives feedback in minutes, not 15+" and "CI wall-clock time (target: <5min for rust-test job)".
