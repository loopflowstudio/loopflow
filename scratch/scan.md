# Scan — 2026-02-17

## What shipped

Four low-risk security and maintenance fixes:

1. **Removed `atty` dependency** — dead code with RUSTSEC advisories (RUSTSEC-2021-0145, RUSTSEC-2024-0375). Replaced with `std::io::IsTerminal`.
2. **Replaced `serde_yaml` with `serde_yaml_ng`** — archived crate (RUSTSEC-2024-0320). Drop-in fork, API-compatible. Chose `serde_yaml_ng` over `serde_yml` (which has RUSTSEC-2025-0068).
3. **Bumped pydantic minimum to `>=2.4.0`** — closes CVE-2024-3772 (ReDoS). Lockfile already resolves to 2.10.6+; this prevents future installs on vulnerable versions.
4. **Pinned GitHub API version header** — added `X-GitHub-Api-Version: 2022-11-28` to both call sites for defensive versioning.

Also restructured scan steps: merged `scan/cves`, `scan/deps`, `scan/upstream` into `scan/scan-report` + `scan/scan-plan`. Added builtin flow listing to `lf list`. Fixed prompt log paths for namespaced steps.

## Deferred upgrades

Each warrants its own PR — multi-file migrations with breaking API changes, no active CVEs at pinned versions.

### Heavy (breaking API changes, heavy usage)
- **git2 0.18 -> 0.20** — libgit2 version bump, heavy usage in lfd
- **rusqlite 0.31 -> 0.38** — seven breaking versions, heavy usage in storage backend
- **bollard 0.17 -> 0.19** — deprecated structs removed, Docker API schema jump. Pair with Docker Engine API v1.44+ minimum
- **axum 0.7 -> 0.8** — path param syntax `/:param` -> `/{param}`, WebSocket API change
- **reqwest 0.12 -> 0.13** — default TLS changed to rustls, feature flag changes
- **tower-http 0.5 -> 0.6** — couples with axum upgrade
- **deadpool-postgres 0.12 -> 0.14** — 0.x semver breaks

### Low priority (no urgency)
- **rand 0.8 -> 0.10** — light usage, breaking API
- **tiktoken-rs 0.6 -> 0.9** — moderate usage, likely breaking
- **portable-pty 0.8 -> 0.9** — moderate usage, likely breaking
- **rich 13.x -> 14.x** — light usage, minor `NO_COLOR`/`FORCE_COLOR` behavior change

## Clean dependencies

No CVEs, current versions — no action needed:

**Rust**: clap, serde, serde_json, tokio, chrono, sha2, hmac, subtle, uuid, tokio-postgres, tokio-stream, tokio-util, futures-util, bytes, ctrlc, once_cell, dirs, ignore, hex, gethostname, thiserror, anyhow, async-trait, cron, time, tempfile, tracing, tracing-subscriber

**Python**: httpx, typer, pyyaml, boto3 (dev)

**Swift**: ViewInspector

**APIs**: Anthropic Messages (v2023-06-01), GitHub REST (v2022-11-28) — both current, no breaking changes
