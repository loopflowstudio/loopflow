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
- **Over-decomposition.** Splitting traits/modules too far creates indirection without value. Pass 1 proved this: the harness command builder trait added a registry, a trait, and 5 files for what a match block and a helper function handle cleanly. The store should be 4-5 focused traits, not 15 micro-traits.
- **Chasing peers.** opencode and convex made different tradeoffs for different reasons. Adopt patterns that fit loopflow's delegation model, not patterns that fight it.
- **Stale reference blast radius.** Direction taxonomy restructuring required three gate iterations to catch all stale references across docs, Swift previews, and wave configs. Pass 1 structural renames (docker module path, store capability imports) had narrower blast radius than expected — the `AgentExecutor` surface isolation helped.
- **Metadata convention drift in docker recovery.** Pass 1 decomposition revealed that recovery correctness depends on label/mount conventions staying consistent across image, workspace, and recovery modules. These conventions are implicit. Pass 2 invariant tests should make them explicit.

## Roadmap (4 passes)

Deep-review findings shifted priority toward deconcentrating hotspot files before adding more feature surface.

| Pass | Phase doc | Scope | What it unlocks | Status |
|---|---|---|---|---|
| 1 | *(shipped)* | Core boundary cleanup (`store` + `docker` + harness commands) | Lower blast radius in hotspot files; deconcentrate docker lifecycle | Done |
| 2 | *(in progress)* | Contract hardening (prompt pipeline split + SQL catalog validation + recovery invariants tests) | Safer iteration on prompt/token policy and fewer runtime contract regressions | In progress |
| 3 | `03-orchestration-expansion.md` | Orchestration expansion (push triggers + flow enrichment) | Faster reactions and richer wave composition once core boundaries are stable | Later |
| 4 | `04-lfd-direction-aliases.md` | lfd-managed direction aliases (sqlite + HTTP API + lfq) | Personal direction presets without repo coupling | Later |

### Pass 1 retrospective

Shipped docker lifecycle decomposition (`docker/{image,workspace,recovery,io}`) and store capability accessors. `AgentExecutor` surface unchanged.

What went as planned: docker lifecycle split landed cleanly — modules absorb their own imports and the orchestration surface in `mod.rs` shrank. Store capability accessors (`wave_state()`, `execution()`, `sessions()`, `admin()`) reduced forwarding boilerplate.

What was revised: harness `HarnessCommandBuilder` trait was over-engineered for 4 model types. Simplified back to a match block with an `apply_harness_env` helper — same behavior, less indirection. A trait earns its keep when there are many implementations or when callers need polymorphism; neither applied here.

What was partial: store backend `match` dispatch still exists inside trait impls. Capability accessors didn't eliminate the backend-port adapter surface. This remaining work feeds into Pass 2.

What we learned: docker recovery logic is still high-risk even after decomposition — the problem is metadata convention drift, not file size. Recovery invariant tests (already planned for Pass 2) are higher priority than originally estimated.

## Reference report

- `00-architecture-report.md` — unified architecture + fragility + four-angle analysis (canonical)

### Findings rolled into roadmap

- Session harness trait work is already shipped (`lfd/sessions/harness/mod.rs`); remove it from future infra debt lists.
- Quality directions are now shipped via direction taxonomy restructuring. Role-style directions (`infra-engineer`, `designer`, `product-engineer`) replaced with composable quality-focused groups (`infra/`, `ux/`, `craft/`, `creativity/`, `ceo/`). Gate and review steps updated with quality-language. Architecture report recommendations #2 (quality-tagged frontmatter) and #4 (API-boundary prompts) remain open.
- `build.rs` codegen pattern is proven for compile-time discovery and validation. BFS expansion with dedup, compile-time directory scanning, `LazyLock<HashMap>` generation — all battle-tested. The same approach applies to Pass 2's SQL catalog validation.
- Direction work was additive to `flow.rs`/`fork.rs`/`prompt.rs` — hotspot files from Pass 1 (`docker.rs`, `store/mod.rs`) were not disturbed. Confirms Pass 1 sequencing.
- Stale references were the biggest friction source. Gate caught stale direction names in docs, Swift preview data, and wave configs across three iterations. Pass 1 renames had narrower blast radius than direction renames — `AgentExecutor` trait surface insulated callers from the docker module split.
- Core risk moved from “too many features” to “too much responsibility in a few files.” Pass 1 addressed the worst hotspots; remaining concentration is in `prompt.rs` and store backend dispatch.
- Baseline guardrails (hotspot concentration + forwarding-surface tracking) apply across all four passes.
- **Pass 1 shipped.** Docker lifecycle decomposition and store capability accessors landed. Harness command trait was tried and simplified back to match dispatch — over-decomposition for 4 model types. Store backend-port adapter extraction is partial — remaining `match` dispatch feeds into Pass 2. Known flaky test: `wave_worktree_tests::wave_rename_renames_branch` (unrelated, intermittent).
