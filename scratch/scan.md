# Scan fixes

## Changes

### 1. Remove dead `atty` dependency
- **Why**: RUSTSEC-2021-0145 / RUSTSEC-2024-0375 (unmaintained, unsoundness on Windows). Also: zero call sites — it's dead code in Cargo.toml.
- **What**:
  - Delete `atty = "0.2"` from `rust/loopflow/Cargo.toml` (line 53)
  - Run `cargo check` to confirm no compile errors
  - The lockfile entry disappears automatically on next resolve
- **Risk**: None. No code references `atty` anywhere. Pure deletion.

### 2. Replace `serde_yaml` with `serde_yaml_ng`
- **Why**: RUSTSEC-2024-0320 — crate archived by dtolnay, no future security patches. Transitive dep `yaml-rust` also unmaintained. `serde_yaml_ng` is the maintained fork with API-compatible surface.
- **What**:
  - In `rust/loopflow/Cargo.toml`: replace `serde_yaml = "0.9"` with `serde_yaml_ng = "0.9"`
  - In all .rs files, replace `serde_yaml::` with `serde_yaml_ng::` and `use serde_yaml` with `use serde_yaml_ng`:
    - `rust/loopflow/src/engine/config.rs` — heaviest user (from_str, from_value, Value, Mapping)
    - `rust/loopflow/src/engine/flow.rs` — from_str, Value, Mapping
    - `rust/loopflow/src/lf/discovery.rs` — from_str, Value
    - `rust/loopflow/src/lfd/config.rs` — from_str
    - `rust/loopflow/src/lfd/http/routes/wave_schemas.rs` — from_str
    - `rust/loopflow/tests/golden_prompt.rs` — from_str
  - Run `cargo fmt`, `cargo clippy -- -D warnings`, `cargo test --all`
- **Risk**: Low. `serde_yaml_ng` is a near drop-in fork. The API surface we use (from_str, from_value, Value, Mapping, Sequence) is identical. Verify with `cargo test --all`. Avoid `serde_yml` (has RUSTSEC-2025-0068).

### 3. Bump pydantic minimum to `>=2.4.0`
- **Why**: CVE-2024-3772 (CVSS 5.9, ReDoS via email validation regex). Affects pydantic 2.0–2.3.x.
- **What**:
  - In `pyproject.toml` line 23: change `"pydantic>=2.0"` to `"pydantic>=2.4.0"`
  - Run `uv lock` to re-resolve
  - Run `uv run pytest python/tests/`
- **Risk**: Near zero. Our lockfile already resolves to 2.10.6+ (well past the fix). We don't use email validation at all — this is purely a floor bump to prevent future installs on vulnerable versions. No code changes needed.

### 4. Pin GitHub API version header
- **Why**: Defensive versioning. GitHub REST API v2022-11-28 is current, but unpinned requests can silently change behavior.
- **What**:
  - In `rust/loopflow/src/lfd/github.rs`: add `X-GitHub-Api-Version: 2022-11-28` header to outgoing requests
  - Find the request builder (likely reqwest) and add `.header("X-GitHub-Api-Version", "2022-11-28")`
- **Risk**: Low. This pins current behavior we already rely on. Verify GitHub CI checks still pass.

## Out of scope

### Heavy dependency upgrades (defer to dedicated PRs)
These are real but each is a multi-file migration with breaking API changes. They don't have deadlines or active CVEs at our pinned versions. Each warrants its own PR:

- **git2 0.18 -> 0.20** — heavy usage in lfd, libgit2 version bump, needs thorough testing of all git operations
- **rusqlite 0.31 -> 0.38** — seven breaking versions, heavy usage in storage backend, cumulative API changes
- **bollard 0.17 -> 0.19** — heavy usage in Docker executor, deprecated structs removed, Docker API schema jump. Should be paired with Docker Engine API v1.44+ minimum change.
- **axum 0.7 -> 0.8** — heavy usage in lfd HTTP/WS server, path param syntax change, WebSocket API change
- **reqwest 0.12 -> 0.13** — heavy usage across Anthropic/GitHub/Studio clients, TLS default change, feature flag changes
- **tower-http 0.5 -> 0.6** — moderate usage, couples with axum upgrade
- **deadpool-postgres 0.12 -> 0.14** — moderate usage, 0.x semver breaks

### Low-priority updates (no urgency)
- **rand 0.8 -> 0.10** — light usage, breaking API changes, no security concern
- **tiktoken-rs 0.6 -> 0.9** — moderate usage, likely breaking, no security concern
- **portable-pty 0.8 -> 0.9** — moderate usage, likely breaking, no security concern
- **rich 13.x -> 14.x** — light usage, minor behavior change with NO_COLOR/FORCE_COLOR env vars
