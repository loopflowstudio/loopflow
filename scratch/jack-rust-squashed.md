# Design: lfd HTTP boundary + http.rs decomposition + command error helper

## Goal
Define a focused refactor that:
- Separates lfd domain models from HTTP API responses via DTOs.
- Decomposes `rust/lfd/src/http.rs` into smaller, focused modules.
- Centralizes shell command error reporting to improve diagnostics across crates.

## Non-goals
- No behavior changes to the HTTP API surface yet.
- No new gRPC work.
- No persistence schema changes.

## Current issues
- HTTP handlers return domain types directly (`types::*`), conflating storage and transport.
- `http.rs` is a large, mixed-responsibility file (>1k LOC).
- Many shell-outs return generic errors without command context.

## Proposal

### 1) LFD transport/domain boundary (DTO layer)
**Add DTOs for HTTP responses and requests** in `rust/lfd/src/http/`.

**Plan:**
- Introduce `http/dto.rs` (or `http/types.rs`) with explicit structs used by HTTP handlers.
- Map domain models (`crate::types::*`) to DTOs via `From` impls or helper functions.
- Keep `crate::types` as storage/domain only; never serialize them directly in HTTP handlers.

**Naming:**
- `WaveDto` or `WaveResponse` are both acceptable. Default to `WaveDto` unless a route benefits from response-specific naming.

**Example sketch:**
```rust
pub struct WaveDto { /* fields serialized */ }

impl From<Wave> for WaveDto { ... }
```

**Benefits:**
- Clean boundary for storage evolution vs API stability.
- Enables validation/formatting without mutating storage models.

### 2) Decompose `http.rs`
Split into 3–5 modules with clear responsibilities.

**Proposed structure:**
```
rust/lfd/src/http/
  mod.rs           # router + shared types
  state.rs         # HttpState + constructor
  dto.rs           # request/response DTOs
  routes/          # handler modules
    waves.rs
    runs.rs
    agents.rs
    hooks.rs
```

**Routing pattern:**
- `http/mod.rs` owns `router(...)` and wires routes from `routes::*`.
- Each route module contains request structs + handlers for its area.

**Benefits:**
- Handler logic is discoverable by area.
- Easier to review & test.
- Smaller diff surfaces for future changes.

### 3) Centralize command execution errors
Many modules shell out to `git`, `gh`, `pbcopy`, `open`, etc.

**Plan:**
- Add a small helper in `loopflow-engine` (base layer) to execute commands and return annotated errors.
- Use a consistent error type (e.g., `CommandError`) with fields:
  - `command`, `args`, `stderr`, `status`
- Replace ad-hoc error formatting in ops/engine where reasonable.

**Example sketch:**
```rust
pub fn run_command(cmd: &mut Command, name: &str) -> Result<Output, CommandError> { ... }
```

**Benefits:**
- Consistent error messages across CLI and daemon.
- Easier to debug failures in ops workflows.

## Risks
- DTO mapping adds boilerplate; must keep HTTP schema consistent.
- Modularizing `http.rs` risks breaking imports and routing if not carefully staged.
- Error helper must not obscure raw stderr when it matters.

## Migration plan
1) Introduce DTOs and conversion helpers.
2) Move handlers into `routes/` with minimal logic changes.
3) Replace direct domain serialization with DTOs in handlers.
4) Add command error helper and migrate high-traffic call sites first (ops + engine).

