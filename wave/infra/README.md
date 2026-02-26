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
- **Metadata convention drift in docker recovery.** Pass 1 decomposition revealed that recovery correctness depends on label/mount conventions staying consistent across image, workspace, and recovery modules. Pass 2 made these explicit via shared constants and invariant tests. Remaining risk: daemon-restart e2e coverage beyond unit tests.
- **Release bootstrap heuristics can drift.** `lf ops release` bootstrap gating currently detects workflow triggers with lightweight string checks (`tags:` + `{prefix}v*`). Fast and simple, but susceptible to unusual YAML layouts.

## Roadmap (4 passes)

Deep-review findings shifted priority toward deconcentrating hotspot files before adding more feature surface.

| Pass | Phase doc | Scope | What it unlocks | Status |
|---|---|---|---|---|
| 1 | *(shipped)* | Core boundary cleanup (`store` + `docker` + harness commands) | Lower blast radius in hotspot files; deconcentrate docker lifecycle | Done |
| 2 | *(shipped)* | Contract hardening (prompt stage newtypes + Docker metadata constants + SQL catalog invariants + golden prompts) | Safer iteration on prompt/token policy and fewer runtime contract regressions | Done |
| 3 | *(shipped)* | Orchestration expansion (activation ingress + push/listen + multi-step fork branches) | Faster reactions and richer wave composition once core boundaries are stable | Done |
| 4 | `04-lfd-direction-aliases.md` | lfd-managed direction aliases (sqlite + HTTP API + lfq) | Personal direction presets without repo coupling | Next |

### Shipped side milestones

- `05-release-general.md` — generalized `lf ops release` for bootstrap-from-zero repos, target-scoped monorepo releases, inline manifest version bumps, and post-tag workflow/release status visibility.
- `05-supported-harnesses-models.md` — standardized `harness:model` naming and added shared model-override controls across engine, lfd, and Concerto.

### Pass 1 retrospective

Shipped docker lifecycle decomposition (`docker/{image,workspace,recovery,io}`) and store capability accessors. `AgentExecutor` surface unchanged.

What went as planned: docker lifecycle split landed cleanly — modules absorb their own imports and the orchestration surface in `mod.rs` shrank. Store capability accessors (`wave_state()`, `execution()`, `sessions()`, `admin()`) reduced forwarding boilerplate.

What was revised: harness `HarnessCommandBuilder` trait was over-engineered for 4 model types. Simplified back to a match block with an `apply_harness_env` helper — same behavior, less indirection. A trait earns its keep when there are many implementations or when callers need polymorphism; neither applied here.

What was partial: store backend `match` dispatch still exists inside trait impls. Capability accessors didn't eliminate the backend-port adapter surface. This remaining work feeds into Pass 2.

What we learned: docker recovery logic is still high-risk even after decomposition — the problem is metadata convention drift, not file size. Recovery invariant tests (already planned for Pass 2) are higher priority than originally estimated.

### Pass 2 retrospective

Shipped prompt stage newtypes, Docker metadata constants + invariant tests, SQL catalog completeness checks, and golden prompt conformance fixtures.

What went as planned: Docker invariant tests landed cleanly and directly address the metadata convention drift risk identified in Pass 1. SQL catalog checks cover both SQLite and Postgres dialects with placeholder validation. Golden prompt fixtures for implement/review/debug establish regression baselines.

What was revised: prompt pipeline used newtypes (`GatheredContext`, `BudgetedContext`, `RenderedPrompt`) rather than a full structural split of `prompt.rs`. Lower refactor cost, same ordering guarantees. The “prompt pipeline split” in the roadmap was really “prompt pipeline contracts” — the file stays large but the ordering is enforced. Store backend `match` dispatch (flagged as partial work in Pass 1) was deliberately retained and documented as intentional — one greppable dispatch point beats macro indirection for the current number of backends.

What remains: `prompt.rs` concentration risk is documented but not blocking. Docker recovery would benefit from daemon-restart e2e tests beyond the unit invariant tests shipped here. Store dispatch verbosity is a conscious tradeoff, not debt — revisit only if backend count grows.

What we learned: contract hardening via invariant tests (catalog completeness, metadata constants, stage newtypes) is high-ROI relative to structural decomposition. The contracts catch drift at compile/test time without the indirection cost of splitting files further. Pass 3 moved from “unblocked” to active implementation with Milestone A shipped.

### Pass 3 retrospective (Milestone A)

Shipped unified activation ingress and queue policy across watch/cron/loop/manual/listen, plus push hook ingestion and activation observability.

What shipped: `triggers/activation.rs` now owns enqueue/coalesce/drop/dispatch policy; watch and cron pollers use it as a shared ingress; loop/manual/listen activations route through the same path. Added `/hooks/git` and `/v0/hooks/github` push ingestion, activation audit storage, run linkage, websocket activation events, and `GET /v0/waves/{wave_id}/activations`.

What was revised: activation queue semantics are now explicit (stimulus-level dedupe, per-wave queue cap defaults, immutable activation outcome log) instead of ad-hoc trigger-specific behavior.

What remains: Pass 3 complete. Original `when` predicates and decision persistence deferred — stimulus→flow routing (shipped in chords wave) covers the reactive flow use case at the wave level instead.

### Pass 3 retrospective (Milestone B)

Shipped multi-step fork branches. Fork branches can now reference named flows and run multiple steps sequentially within a single worktree. Both CLI (`lf flow`) and daemon (`lfd`) executors support this.

What shipped: `ConcreteForkBranch` struct with `Vec<ConcreteStep>`, `flow:` key in fork YAML parsing, `expand_fork` resolving flow references into multi-step branches (rejecting nested forks), `ForkManifestBranch.steps: Vec<ForkManifestStep>` with per-step exit codes, fail-fast execution in both executors, and `is_multi_step_flow()` extraction to deduplicate the flow-expansion heuristic.

What went as planned: the type-level design (`ConcreteForkBranch` wrapping steps + directions + label) kept the executor changes mechanical. The three YAML formats (`step:` shorthand, `flow:` shorthand, explicit `branches:`) parsed cleanly with backwards compatibility. All 61 existing flow/fork tests passed unchanged.

What was deferred: conditional flow nodes (`when` predicates), activation payload persistence, decision persistence/replay, and decision observability. These were scoped in the original `03-orchestration-expansion.md` but cut during milestone scoping. Stimulus→flow routing at the wave level (shipped in chords wave) covers the primary reactive use case.

What we learned: manifest schema changes are safe because manifests are ephemeral (written per-run, consumed by synthesize). The `lf flow` CLI error message doesn't name the specific failed step in a multi-step branch — daemon executor logs step-level detail via tracing. Minor UX gap, not blocking.

## Reference report

- `00-architecture-report.md` — unified architecture + fragility + four-angle analysis (canonical)

### Findings rolled into roadmap

- Session harness trait work is already shipped (`lfd/sessions/harness/mod.rs`); remove it from future infra debt lists.
- Quality directions are now shipped via direction taxonomy restructuring. Role-style directions (`infra-engineer`, `designer`, `product-engineer`) replaced with composable quality-focused groups (`infra/`, `ux/`, `craft/`, `creativity/`, `ceo/`). Gate and review steps updated with quality-language. Architecture report recommendations #2 (quality-tagged frontmatter) and #4 (API-boundary prompts) remain open.
- `build.rs` codegen pattern is proven for compile-time discovery and validation. BFS expansion with dedup, compile-time directory scanning, `LazyLock<HashMap>` generation — all battle-tested. The same approach applies to Pass 2's SQL catalog validation.
- Direction work was additive to `flow.rs`/`fork.rs`/`prompt.rs` — hotspot files from Pass 1 (`docker.rs`, `store/mod.rs`) were not disturbed. Confirms Pass 1 sequencing.
- Stale references were the biggest friction source. Gate caught stale direction names in docs, Swift preview data, and wave configs across three iterations. Pass 1 renames had narrower blast radius than direction renames — `AgentExecutor` trait surface insulated callers from the docker module split.
- Core risk moved from “too many features” to “too much responsibility in a few files.” Pass 1 addressed the worst hotspots; Pass 2 hardened contracts in the remaining concentrations (`prompt.rs` via stage newtypes, store via catalog invariants). `prompt.rs` concentration remains but is now contractually constrained.
- Baseline guardrails (hotspot concentration + forwarding-surface tracking) apply across all four passes.
- **Pass 1 shipped.** Docker lifecycle decomposition and store capability accessors landed. Harness command trait was tried and simplified back to match dispatch — over-decomposition for 4 model types.
- **Pass 2 shipped.** Prompt stage newtypes, Docker metadata constants + invariant tests, SQL catalog completeness checks, golden prompt fixtures. Store backend `match` dispatch deliberately retained — documented as intentional single dispatch point, not accidental boilerplate. Known flaky test: `wave_worktree_tests::wave_rename_renames_branch` (unrelated, intermittent).
- **Pass 3 Milestone A shipped.** Activation ingress is centralized with push/listen/manual coverage, queue/audit observability, and explicit drop/coalesce semantics. Flow-language Milestone B remains.
- **Agent naming side track shipped.** `05-supported-harnesses-models.md` unified terminology to `agent` (harness:model pairs), added optional user `agent` config + step `default_agent`, and wired wave/Concerto agent overrides. Carry-forward items: tri-state PATCH semantics, per-step override editing UI, and local macOS UI-test runner stability.
