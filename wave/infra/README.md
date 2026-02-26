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
- **Over-decomposition.** Splitting traits/modules too far creates indirection without value. A trait earns its keep when there are many implementations or when callers need polymorphism. The store should be 4-5 focused traits, not 15 micro-traits.
- **Chasing peers.** opencode and convex made different tradeoffs for different reasons. Adopt patterns that fit loopflow's delegation model, not patterns that fight it.
- **Stale reference blast radius.** Structural renames have narrower blast radius when trait surfaces insulate callers. Direction/taxonomy renames are wider — gate caught stale names across docs, Swift previews, and wave configs.
- **Metadata convention drift in docker recovery.** Recovery correctness depends on label/mount conventions staying consistent across image, workspace, and recovery modules. Shared constants and invariant tests mitigate this, but daemon-restart e2e coverage remains a gap.
- **Release bootstrap heuristics can drift.** `lf ops release` bootstrap gating uses lightweight string checks (`tags:` + `{prefix}v*`). Simple, but susceptible to unusual YAML layouts.

## Reference report

- `00-architecture-report.md` — unified architecture + fragility + four-angle analysis (canonical)
