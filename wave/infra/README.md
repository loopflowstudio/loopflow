# Infra

Internal architecture, efficiency, and code quality. Keep the codebase compact and high-leverage as it grows.

## Vision

loopflow is a "smart router" — 91k lines orchestrating coding agents, not reimplementing their capabilities. The architecture should stay thin: centralize state in lfd, delegate to agents, extend via files. Every line earns its place.

### Not here

- New features or product capabilities (those belong in their respective waves)
- External API design (see agentapi)
- Performance optimization without profiling data

## Goals

- Maintain architectural compactness as features grow
- Eliminate boilerplate and duplicated patterns
- Invest in the prompt engine and flow system (the differentiators)
- Make extension points trait-based, not switch-based

## Risks

- **Abstraction creep.** Refactoring for elegance can add lines instead of removing them. Every change should net-reduce or hold steady on LOC.
- **Over-decomposition.** Splitting traits/modules too far creates indirection without value. The store should be 4-5 focused traits, not 15 micro-traits.
- **Chasing peers.** opencode and convex made different tradeoffs for different reasons. Adopt patterns that fit loopflow's delegation model, not patterns that fight it.

## Roadmap (3 passes)

Deep-review findings shifted priority toward deconcentrating hotspot files before adding more feature surface.

| Pass | Phase doc | Scope | What it unlocks | Status |
|---|---|---|---|---|
| 1 | `01-core-boundary-cleanup.md` | Core boundary cleanup (`store` + `docker` + provider registry) | Lower blast radius in hotspot files; add providers without central switch edits | Next |
| 2 | `02-contract-hardening.md` | Contract hardening (prompt pipeline split + SQL catalog validation + recovery invariants tests) | Safer iteration on prompt/token policy and fewer runtime contract regressions | After Pass 1 |
| 3 | `03-orchestration-expansion.md` | Orchestration expansion (push triggers + flow enrichment) | Faster reactions and richer wave composition once core boundaries are stable | Later |

## Reference report

- `00-architecture-report.md` — unified architecture + fragility + four-angle analysis (canonical)

### Findings rolled into roadmap

- Session harness trait work is already shipped (`lfd/sessions/harness/mod.rs`); remove it from future infra debt lists.
- Quality directions are now shipped via direction taxonomy restructuring. Role-style directions (`infra-engineer`, `designer`, `product-engineer`) replaced with composable quality-focused groups (`infra/`, `ux/`, `values/`). Gate and review steps updated with quality-language. Architecture report recommendations #2 (quality-tagged frontmatter) and #4 (API-boundary prompts) remain open.
- Core risk moved from “too many features” to “too much responsibility in a few files.”
- Baseline guardrails (hotspot concentration + forwarding-surface tracking) apply across all three passes.
