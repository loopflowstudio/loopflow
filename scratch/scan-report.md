# Scan Report — 2026-02-17

## Vulnerabilities

### atty — RUSTSEC-2021-0145 / RUSTSEC-2024-0375
- **Severity**: low (unsoundness on Windows with custom allocator)
- **Current version**: 0.2.14
- **Fixed in**: no fix — crate is permanently unmaintained
- **Summary**: Potential unaligned pointer read on Windows. Crate abandoned with no planned releases.
- **Source**: https://rustsec.org/advisories/RUSTSEC-2024-0375.html
- **Our usage**: direct dependency in `rust/loopflow/Cargo.toml`
- **Recommendation**: Replace with `std::io::IsTerminal` (stable since Rust 1.70, zero dependencies)

### serde_yaml — RUSTSEC-2024-0320 (transitive: yaml-rust unmaintained)
- **Severity**: informational (archived, no future security patches)
- **Current version**: 0.9.34+deprecated
- **Fixed in**: N/A — crate archived by dtolnay on 2024-03-25
- **Summary**: Repository archived permanently. Transitive dep `yaml-rust` also unmaintained. No security patches will be issued.
- **Source**: https://github.com/dtolnay/serde-yaml
- **Our usage**: direct dependency in `rust/loopflow/Cargo.toml`
- **Recommendation**: Replace with `serde_yaml_ng` (maintained fork, near drop-in replacement). Avoid `serde_yml` (has RUSTSEC-2025-0068 for unsoundness).

### pydantic — CVE-2024-3772
- **Severity**: medium (CVSS 5.9)
- **Current version**: >=2.0 (allows vulnerable 2.0–2.3.x)
- **Fixed in**: 2.4.0
- **Summary**: ReDoS via crafted email string in email validation regex.
- **Source**: https://nvd.nist.gov/vuln/detail/CVE-2024-3772
- **Our usage**: direct dependency in `pyproject.toml`
- **Recommendation**: Bump minimum to `>=2.4.0`

No critical or high severity CVEs affect any dependency at its pinned version. The git2/libgit2-sys CVE-2024-24575/CVE-2024-24577 (high) is already resolved by the lockfile pin at libgit2-sys 0.16.2. The tokio RUSTSEC-2025-0023 is resolved by the lockfile pin at tokio 1.49.0.

## Stale dependencies

### serde_yaml: 0.9.34 -> archived (replace)
- **Type**: archived/deprecated
- **Breaking changes**: N/A — replacement `serde_yaml_ng` is API-compatible
- **Migration guide**: https://github.com/acatton/serde-yaml-ng
- **Our usage**: moderate (YAML config parsing throughout engine)

### atty: 0.2.14 -> std::io::IsTerminal (replace)
- **Type**: unmaintained, superseded by stdlib
- **Breaking changes**: API change (`atty::is(Stream::Stdout)` -> `std::io::stdout().is_terminal()`)
- **Migration guide**: https://rustsec.org/advisories/RUSTSEC-2024-0375.html
- **Our usage**: light (TTY detection)

### git2: 0.18.3 -> 0.20.4
- **Type**: two major releases behind, security advisory on older libgit2
- **Breaking changes**: yes — libgit2 requirement bumped from 1.7.x to 1.9.x
- **Migration guide**: https://github.com/rust-lang/git2-rs/blob/master/CHANGELOG.md
- **Our usage**: heavy (git operations in lfd)

### rusqlite: 0.31.0 -> 0.38.0
- **Type**: seven breaking versions behind
- **Breaking changes**: yes — cumulative API changes across 0.32–0.38
- **Migration guide**: https://crates.io/crates/rusqlite
- **Our usage**: heavy (SQLite storage backend)

### rand: 0.8.5 -> 0.10.0
- **Type**: two major releases behind
- **Breaking changes**: yes — `thread_rng()` renamed to `rng()`, `gen` methods renamed, `Uniform::new` returns `Result`
- **Migration guide**: https://rust-random.github.io/book/update-0.9.html
- **Our usage**: light

### tiktoken-rs: 0.6.0 -> 0.9.1
- **Type**: three major releases behind
- **Breaking changes**: likely
- **Migration guide**: https://github.com/zurawiki/tiktoken-rs
- **Our usage**: moderate (token counting)

### bollard: 0.17.1 -> 0.19.4
- **Type**: two major releases behind
- **Breaking changes**: yes — deprecated option structs removed in 0.19, Docker API schema jumped to 1.49+
- **Migration guide**: https://github.com/fussybeaver/bollard/releases
- **Our usage**: heavy (Docker executor)

### axum: 0.7.9 -> 0.8.6
- **Type**: one major release behind
- **Breaking changes**: yes — path params `/:param` -> `/{param}`, WebSocket `Message` uses `Bytes` instead of `Vec<u8>`, `get_service` removed
- **Migration guide**: https://tokio.rs/blog/2025-01-01-announcing-axum-0-8-0
- **Our usage**: heavy (lfd HTTP/WS server)

### reqwest: 0.12.28 -> 0.13.2
- **Type**: one major release behind
- **Breaking changes**: yes — default TLS changed to rustls, `query()` and `form()` now opt-in features
- **Migration guide**: https://seanmonstar.com/blog/reqwest-v013-rustls-default/
- **Our usage**: heavy (HTTP client for Anthropic, GitHub, Loopflow Studio)

### tower-http: 0.5.2 -> 0.6.8
- **Type**: one major release behind
- **Breaking changes**: yes (0.x semver)
- **Migration guide**: https://crates.io/crates/tower-http
- **Our usage**: moderate (tracing middleware)

### deadpool-postgres: 0.12.1 -> 0.14.1
- **Type**: two major releases behind
- **Breaking changes**: yes (0.x semver)
- **Migration guide**: https://crates.io/crates/deadpool-postgres
- **Our usage**: moderate (Postgres connection pool)

### portable-pty: 0.8.1 -> 0.9.0
- **Type**: one major release behind
- **Breaking changes**: likely
- **Migration guide**: https://crates.io/crates/portable-pty
- **Our usage**: moderate (PTY allocation for agents)

### rich: 13.x -> 14.3.2 (Python)
- **Type**: major release
- **Breaking changes**: `NO_COLOR=""` and `FORCE_COLOR=""` now treated as unset (was treated as enabled). Minor behavior change.
- **Migration guide**: https://github.com/Textualize/rich/releases
- **Our usage**: light (CLI output formatting via lfq)

## Upstream changes

### Anthropic Messages API — no changes
- **What changed**: Nothing. `anthropic-version: 2023-06-01` remains the current and only production API version.
- **Affects**: `rust/loopflow/src/agent/anthropic.rs`, `swift/Concerto/Services/AnthropicClient.swift`
- **Deadline**: none
- **Migration**: none needed. New capabilities added via beta headers, not API versions.

### GitHub REST API — no breaking changes, version pinning recommended
- **What changed**: API version `2022-11-28` remains current. No deprecations on check-runs or pulls endpoints.
- **Affects**: `rust/loopflow/src/lfd/github.rs`
- **Deadline**: none
- **Migration**: Consider adding `X-GitHub-Api-Version: 2022-11-28` header to pin behavior defensively.

### Docker Engine API — minimum version raised to v1.44
- **What changed**: Docker Engine v29 raised minimum supported API version to v1.44. Bollard 0.17 targets v1.45, so it still connects, but misses newer response fields from v1.46–v1.52.
- **Affects**: `rust/loopflow/src/lfd/executor/docker.rs`
- **Deadline**: no hard deadline, but Docker Engine v29 is actively rolling out
- **Migration**: Upgrade bollard from 0.17 to 0.19+. Test container lifecycle operations after upgrade.

## Clean

Dependencies and services checked with no issues:

**Rust (no CVEs, current versions)**:
clap 4.5, serde 1.0, serde_json 1.0, tokio 1.x, chrono 0.4, sha2 0.10, hmac 0.12, subtle 2, uuid 1, tokio-postgres 0.7, tokio-stream 0.1, tokio-util 0.7, futures-util 0.3, bytes 1, ctrlc 3, once_cell 1, dirs 5, ignore 0.4, hex 0.4, gethostname 0.4, thiserror 1.0, anyhow 1, async-trait 0.1, cron 0.12, time 0.3, tempfile 3.9, tracing 0.1, tracing-subscriber 0.3

**Python (no CVEs at pinned versions)**:
httpx >=0.27, typer >=0.9, pyyaml >=6.0, boto3 (dev)

**Swift**:
ViewInspector 0.10.3 (current, no issues)

**External APIs (no breaking changes)**:
Anthropic Messages API (v2023-06-01), GitHub REST API (v2022-11-28)
