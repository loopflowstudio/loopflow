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
- **Release bootstrap heuristics can drift.** `lf ops release` bootstrap gating currently detects workflow triggers with lightweight string checks. Fast and simple, but susceptible to unusual YAML layouts.

## Reference

- `00-architecture-report.md` — unified architecture + fragility + four-angle analysis
