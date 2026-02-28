# CI Speed — Review Doc

## What was implemented

Six changes to make CI faster and more reliable:

1. **Rust build caching** — `Swatinem/rust-cache@v2` added to `rust-test`, `e2e-smoke`, and `docker-smoke` jobs in `ci.yml`. Caches compiled dependencies between runs.

2. **Docker dependency layer split** — Dockerfile now copies Cargo manifests first, builds stub sources to compile all 393 deps, then copies real source for incremental rebuild. Source-only changes skip the dep compilation layer entirely.

3. **Ruff lint in CI** — `uv run ruff check` runs in `python-test` before pytest. `ruff>=0.8` added to dev deps. Existing ruff config in pyproject.toml drives the rules. `.agents` directory excluded.

4. **Release build caching** — `Swatinem/rust-cache@v2` with `shared-key: release-${{ matrix.target }}` in `build-native`. Each of the 4 cross-compile targets gets its own cache.

5. **pyproject.toml version check** — `auto-tag.yml` now verifies pyproject.toml version matches RELEASE_NOTES.md, alongside the existing Cargo.toml check. Prevents shipping mismatched Python package versions.

6. **Remove `--allow-dirty`** — `cargo publish` in `release.yml` no longer masks workspace cleanliness issues. GitHub Actions checkout guarantees a clean workspace.

Additionally: ruff autofix applied across the Python codebase (import sorting, line length, unused imports, f-string fixes).

## Key choices

| Decision | Why |
|----------|-----|
| Swatinem/rust-cache, not sccache | Zero config for GitHub Actions. Sccache needs S3/GCS backend. |
| Stub sources, not cargo-chef | 2-member workspace doesn't justify a build tool dependency. Few lines of Dockerfile. |
| Ruff in python-test, not separate job | Ruff runs in ~200ms. Separate job adds 30s+ overhead for checkout + setup. |
| `shared-key` for release matrix | Prevents cache fragmentation across the 4 OS/target combinations. |
| Ruff autofix included in this PR | Adding ruff to CI without fixing existing violations would break the build. Ship together. |

## How it fits together

CI workflow files (`ci.yml`, `release.yml`, `auto-tag.yml`) get caching and checks. The Dockerfile gets a layer split. `pyproject.toml` gets `ruff` as a dev dep and `.agents` exclusion. Python source files get ruff-conformant formatting.

No runtime behavior changes. All changes are build/CI infrastructure.

## Risks and bottlenecks

- **Cache invalidation**: Swatinem/rust-cache hashes on Cargo.lock + rustc version + job name. A Cargo.lock change rebuilds from scratch (correct behavior, but the first run after a dep bump will be slow).
- **Dockerfile stubs must match crate layout**: If a new `[[bin]]` target is added to `rust/loopflow/Cargo.toml`, the Dockerfile stub section needs a matching `echo "fn main() {}" > ...` line. This is documented in the design doc but could be missed.
- **sed pattern for version extraction**: Both Cargo.toml and pyproject.toml version checks use `sed -n 's/^version = "\(.*\)"/\1/p' | head -1`. This works because both files have `version = "X.Y.Z"` early. If a dependency section ever has a `version = "..."` line starting at column 0, the pattern would match it — but `head -1` takes the first match which is the package version.

## What's not included

- macOS/Swift job caching (different ecosystem, separate effort)
- Python dependency caching (uv is already fast)
- Docker image CI builds (docker-smoke tests cargo, not Docker)
- `uv.lock` changes are a consequence of adding `ruff>=0.8` to dev deps
