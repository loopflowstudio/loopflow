status: proposed

# Simplify lfd storage + workspace + prompt assembly without touching executor split

## Context
User intent is to keep both deployment modes while removing avoidable complexity:
- SQLite stays default for local/self-hosted single-node use.
- Postgres remains supported for shared/company deployment.
- The current store layer duplicates SQL and forces async/sync bridging (`run_store`, route-level `spawn_blocking`, Postgres `block_on`).
- Workspace resolution for waves is duplicated across executor/routes using raw strings.
- `engine/prompt.rs` mixes many gathering/formatting paths and uses stringly-typed document categories.

The executor module split is being handled separately and is intentionally out of scope for this wave.

## Scope
In scope (4 opportunities):
1. Store async/sync mismatch cleanup
2. SQLite/Postgres duplication reduction while keeping both backends
3. Wave workspace first-class domain model
4. Prompt/context assembly unification

Out of scope:
- Executor module decomposition (`lfd/executor.rs` split)
- Product behavior changes to wave semantics, flow semantics, or scheduler policy

## Approach
1. **Stage 1 — Store trait scope reset (foundation, low risk)**
   - Replace monolithic store interface with grouped capability traits (`WaveStateStore`, `ExecutionStore`, `StoreAdmin`) and concrete `Store` wrapper.
   - Keep behavior unchanged; preserve both backends.
   - Provide transition shim so call sites compile incrementally.

2. **Stage 2 — Async store boundary migration**
   - Convert capability traits and backend methods to async.
   - Remove `run_store(...)` helper and migrate HTTP routes to direct async store calls.
   - Eliminate store-related route `spawn_blocking` usage.

3. **Stage 3 — Shared SQL catalog for dual backend parity**
   - Introduce canonical query catalog and dialect rendering (`?` -> `$1..$n`).
   - Reduce duplicated SQL bodies in sqlite/postgres implementations.
   - Add converter/query snapshot tests to guarantee SQL parity.

4. **Stage 4 — WaveWorkspace domain object**
   - Add `WaveWorkspace` + `WorkspaceVariant` + `WorkspaceService`.
   - Replace duplicated `ensure_wave_worktree` / `resolve_wave_work_dir*` logic with shared service APIs.
   - Keep persisted `WaveRun.worktree` + `branch`; derive workspace runtime object.

5. **Stage 5 — Prompt pipeline unification**
   - Replace `Document.category: String` with `DocumentSource` enum.
   - Consolidate gather paths under `gather_documents(specs)`.
   - Consolidate formatters into `format_prompt(mode)` with thin compatibility wrappers first.

6. **Stage 6 — Cleanup + parity hardening**
   - Remove temporary compatibility shims.
   - Tighten tests for prompt parity, route behavior, and backend parity.
   - Confirm no `block_on`/`block_in_place` in store request paths.

Each stage is independently shippable and should land as a small, reviewable commit.
