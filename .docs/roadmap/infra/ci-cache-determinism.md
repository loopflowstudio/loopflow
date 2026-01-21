---
status: proposed
area: infra
---

# Deterministic CI caching for Loopflow + Maestro

Our CI runs are reliable but slower than they need to be and still depend on network installs (uv sync, SwiftPM, brew). Add deterministic toolchain pinning and caching to reduce runtime and flake risk, especially for the macOS Maestro jobs.

## Scope

- Cache uv package downloads and build artifacts to speed Python tests.
- Cache SwiftPM and Maestro build outputs for faster macOS jobs.
- Pin Xcode and xcodegen versions to avoid toolchain drift.
- Document cache busting and CI troubleshooting steps.
- Not included: refactoring tests, changing product behavior, or moving to self-hosted runners.

## Approach

Update `.github/workflows/ci.yml` to:
- Add a shared cache step for `uv` (`~/.cache/uv` and any project-local build cache) keyed on `uv.lock`.
- Add Swift caches (`~/.swiftpm`, `Maestro/.build`) keyed on `Maestro/Package.resolved`.
- Use a dedicated Xcode setup action to pin the macOS toolchain version.
- Install `xcodegen` with a pinned version (brew bundle or direct download) and include it in the cache key.
- Add a short CI note in docs describing how to invalidate caches when deps change.
