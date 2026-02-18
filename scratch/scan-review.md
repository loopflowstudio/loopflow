# Scan — Review

## What was implemented

Four low-risk security and maintenance fixes, plus a restructuring of the scan step system:

1. **Removed `atty` dependency** — dead code with RUSTSEC advisories. Replaced with `std::io::IsTerminal` (stable since Rust 1.70).
2. **Replaced `serde_yaml` with `serde_yaml_ng`** — archived crate with no future security patches. Drop-in fork, API-compatible.
3. **Bumped pydantic minimum to `>=2.4.0`** — closes CVE-2024-3772 (ReDoS). Lockfile already resolves past the fix; this prevents future installs on vulnerable versions.
4. **Pinned GitHub API version header** — added `X-GitHub-Api-Version: 2022-11-28` to both GitHub API call sites for defensive versioning.

Additionally:
5. **Consolidated scan steps** — merged three scan steps (`scan/cves`, `scan/deps`, `scan/upstream`) into two (`scan/scan-report`, `scan/scan-plan`). The old split created artificial boundaries between related checks.
6. **Added builtin flow listing** — `lf list` now shows builtin flows by category with descriptions, not just user-defined flows. User overrides are marked "(customized)".
7. **Fixed prompt log path for namespaced steps** — `scan/scan-report` no longer creates subdirectories in `.lf/log/`.

## Key choices

**`serde_yaml_ng` over `serde_yml`**: `serde_yml` has its own RUSTSEC advisory (RUSTSEC-2025-0068). `serde_yaml_ng` is the safe maintained fork.

**Two scan steps instead of three**: The old cves/deps/upstream split forced three separate agent invocations that each re-read the same manifests. Combining into scan-report (gather all findings) and scan-plan (triage into design doc) is a better workflow split — one agent does all the research, another does the planning.

**Builtin flow metadata in discovery.rs**: Descriptions and categories are hardcoded alongside the step metadata that already lives there. This matches the existing pattern for `BUILTIN_CATEGORIES` and `builtin_descriptions()`.

## How it fits together

The dependency changes (atty, serde_yaml, pydantic) are mechanical replacements with no behavioral change. The GitHub header is additive. The scan step restructuring touches builtins.rs (registration), discovery.rs (listing metadata), list.rs (display), and the step markdown files themselves.

## Risks and bottlenecks

**Low risk overall.** All changes are either deletions, drop-in replacements, or additive.

- `serde_yaml_ng` API compatibility is verified by the full test suite passing (341 Rust tests).
- The pydantic bump is a floor raise only — the lockfile already resolves to 2.10.6+.
- GitHub API version pinning matches the version we already implicitly use.

## What's not included

Heavy dependency upgrades (git2, rusqlite, bollard, axum, reqwest, tower-http, deadpool-postgres) are explicitly deferred — each requires a multi-file migration with breaking API changes. See "Out of scope" in `scratch/scan.md` for the full list.
