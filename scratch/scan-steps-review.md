# Scan Steps — Design Review

## What was implemented

Three built-in scan steps that look outward instead of inward:

- **`scan/cves`** — checks dependency manifests for known security vulnerabilities via web search
- **`scan/deps`** — checks for major version bumps, deprecations, and end-of-life notices
- **`scan/upstream`** — checks external APIs and SDKs for breaking changes

Plus a `scan` flow that chains all three, a `scan` wave definition for future daily cron activation, and a roadmap item for the built-in waves Concerto feature.

## Key choices

**Namespaced step names.** Scan steps use `scan/cves` rather than just `cves`. This prevents name collisions (e.g., `deps` is too generic) and groups the steps visually in listings. This is the first step category to use namespace prefixes — all other categories (code, plan, ops) use flat names.

**NAMESPACED_STEPS override in builtins.rs.** The auto-registration via `build.rs` uses file stems as keys, so `steps/scan/cves.md` registers as `"cves"`. The `NAMESPACED_STEPS` map provides the correct `"scan/cves"` keys. `get_builtin_step` checks both maps. An alternative was modifying `build.rs` to use relative paths as keys (changing ALL step keys to `"code/debug"`, etc.), but that would require updating all flow YAMLs and hardcoded references — out of scope for this branch.

**`model: claude:sonnet` in frontmatter.** All three scan steps specify Sonnet for execution. These steps primarily do web searches and summarize findings — cheaper model is appropriate.

**Wave definition without activation.** `waves/scan.yaml` ships as a YAML file but has no registration code. The `BUILTIN_WAVES` registry was removed in a prior commit. Activation will come via the Concerto "available waves" feature described in the roadmap item.

## How it fits together

```
User runs:   lf scan/cves           (or)   lf flow scan
             ↓                              ↓
load_step("scan/cves")              load_flow("scan")
             ↓                              ↓
NAMESPACED_STEPS lookup             flows/scan/scan.yaml
             ↓                              ↓
steps/scan/cves.md                  scan/cves → scan/deps → scan/upstream
             ↓
Agent searches web for CVEs
             ↓
Writes scratch/scan-cves.md
```

## Risks and bottlenecks

- **Web search quality.** Scan steps depend on web search finding relevant CVE databases and changelogs. Results will vary by search provider and recency.
- **Naming convention precedent.** The `scan/` namespace prefix is the first of its kind. Future step categories may want similar treatment, which would require either extending `NAMESPACED_STEPS` or fixing `build.rs` to handle relative paths.
- **No automated validation.** Scan results are advisory — the agent summarizes what it finds but can't verify whether a CVE actually affects the pinned version without running the package manager's audit tool.

## What's not included

- **`build.rs` relative-path keys.** The auto-registration still uses flat file stems. A broader refactor to use `"code/debug"` style keys everywhere is left for a separate branch.
- **Concerto wave activation UI.** The wave YAML is a definition only. The roadmap item (`roadmap/ux/built-in-waves.md`) describes the full activation flow.
- **Package manager audit integration.** Steps use web search rather than `cargo audit` / `npm audit` / etc. Adding tool-based scanning is a natural follow-up.
