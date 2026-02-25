# 01: Core Boundary Cleanup

## Problem

`lfd/store/mod.rs` (1,849 lines), `lfd/executor/docker.rs` (2,839 lines), and `engine/agent.rs` (harness command switching) are concentrated seams where small changes create large regression blast radius.

This pass serves maintainers and contributors first: faster reviews, safer refactors, and simpler harness additions. Users benefit indirectly through fewer orchestration regressions and more predictable wave execution.

Why now: wave pass 1 is explicitly the deconcentration pass before contract-hardening and orchestration expansion. If we skip this, later features compound complexity instead of leverage.

Wave-goal alignment:
- Advances **"Eliminate boilerplate and duplicated patterns"** by removing store forwarder/match repetition.
- Advances **"Make extension points trait-based, not switch-based"** by replacing harness command branching with harness-owned builders.
- Advances **"Maintain architectural compactness as features grow"** by splitting `docker` responsibilities into lifecycle modules.

## Approach

Choose a structural refactor with hard seams, not helper-function cleanup.

### 1) Store boundary: ports over façade forwarding

- Add a first-class `SessionStore` trait and fold session operations into the same dispatch model as wave/execution/admin.
- Introduce backend adapters:
  - `SqliteStoreBackend` (preserves `spawn_blocking` via `run_sqlite`)
  - `PostgresStoreBackend`
- Define a composed backend port:
  - `StoreBackendPort: WaveStateStore + ExecutionStore + SessionStore + StoreAdmin`
- Change `Store` to hold one backend port object and expose capability accessors (`wave_state()`, `execution()`, `sessions()`, `admin()`) instead of dozens of forwarders.
- Migrate call sites to capability traits; remove forwarding-heavy `impl Store` methods once call sites are updated.

Concrete target: `store/mod.rs` no longer contains per-method backend `match` blocks for normal data paths.

### 2) Docker executor: lifecycle modules with one orchestrator

Replace single-file `docker.rs` with `executor/docker/` module tree:

- `mod.rs` — `DockerExecutor` orchestration and `AgentExecutor` impl
- `image.rs` — pull/build/tag/ensure image lifecycle
- `workspace.rs` — volume identity, shared clone, worktree prep/cleanup
- `recovery.rs` — startup reattach, orphan detection/cleanup
- `io.rs` — run/terminate/log streaming/container wait paths
- `types.rs` — shared structs/constants used across modules

Rules:
- no user-visible behavior changes
- preserve existing labels, mount contract, recovery semantics
- keep cross-module data flow explicit via typed structs (`DockerWorkspace`, `RehydrationPlan`, etc.)

Concrete target: no single docker module exceeds ~900 lines.

### 3) Harness command registry: harness-owned builders

- Create `engine/harness_commands/` with a `HarnessCommandBuilder` trait:
  - `build_command(...) -> Vec<String>`
  - `apply_env(...)`
- Move harness-specific command logic into modules:
  - `claude.rs`, `codex.rs`, `gemini.rs`, `opencode.rs`
- Add a registry lookup keyed by parsed harness/backend string; `build_model_command()` becomes a thin delegator.
- Keep unknown-model behavior parity through an explicit fallback builder: unknown models currently default to Claude and pass the full model string as the variant (current `FallbackClaude` behavior).
- Keep the fallback harness choice isolated behind the fallback builder so we can change the default in a later pass without reintroducing central switch logic.
- Standardize terminology on **harness** across engine and sessions where possible. Keep existing persisted field names (for example `provider_session_id`) unchanged in this pass.

Concrete target: adding a harness requires adding a harness module + registration entry, with no central `match` branch growth.

### 4) Mandatory sweep pass after each decomposition

After store, harness, and docker changes, run stale-reference sweeps (code + docs + tests) to catch rename fallout early.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Macro-generate forwarding in `store/mod.rs` and keep current shape | Fewer handwritten lines, but responsibility concentration remains and session dispatch inconsistency survives | Hides pain instead of fixing boundaries |
| Full rewrite of store/executor APIs in one big migration | Clean slate architecture | High semantic risk; violates pass contract of structural-not-semantic change |
| Keep central harness switch and only extract helper functions | Small diff, easy review | Fails wave goal to make extension points trait/registry-based; central branching pressure remains |

## Key decisions

- **Decision: remove forwarding-heavy store façade instead of polishing it.**
  - Why: boilerplate is the problem, not formatting.
- **Decision: split docker by lifecycle ownership, not by technical primitive (git/docker/fs).**
  - Why: lifecycle seams map to failure domains (image/workspace/recovery/IO).
- **Decision: harness modules own command construction and env wiring.**
  - Why: harness-specific behavior should evolve in harness-specific files.
- **Decision: enforce sweep passes as part of implementation, not optional cleanup.**
  - Why: prior direction-taxonomy work proved stale-reference blast radius is real.

Wild success we are designing for:
- new harness onboarding is a self-contained file addition
- docker recovery bugs are fixed in `recovery.rs` without touching workspace/image code
- store changes stop causing multi-domain merge conflicts in one hotspot file

Wild failure we are designing against:
- **Abstraction creep** (more traits/modules but same complexity): mitigate with line-count targets and deleting old façades.
- **Over-decomposition** (indirection tax): cap store capability surfaces to wave/execution/session/admin only.
- **New risk: async boundary mistakes in sqlite adapter** (blocking work accidentally running on async runtime): preserve `run_sqlite` as the only sqlite execution path.

## Scope

- In scope:
  - Store backend port + session trait unification + call-site migration
  - Docker lifecycle module split with unchanged executor contract
  - Harness command registry and harness-owned command/env builders
  - Stale-reference sweep passes after each structural move
- Out of scope:
  - Prompt budgeting or prompt rendering behavior changes
  - Trigger model changes (polling/webhooks)
  - Flow language changes
  - Session API behavior changes
  - New product capabilities (per `wave/infra` Vision “Not here”)

## Done when

- `cargo fmt --all -- --check`
- `cargo clippy -p loopflow --all-targets -- -D warnings`
- `cargo test -p loopflow store_`
- `cargo test -p loopflow docker_`
- `cargo test -p loopflow build_model_command`

And these observable outcomes hold:
- `store/mod.rs` no longer carries forwarding-heavy normal call paths (wave goal: **"Eliminate boilerplate and duplicated patterns"**).
- Docker executor logic is split into lifecycle-owned modules with `AgentExecutor` behavior parity (wave goal: **"Maintain architectural compactness as features grow"**).
- Harness command construction is registry/trait based, not switch growth in central code (wave goal: **"Make extension points trait-based, not switch-based"**).
