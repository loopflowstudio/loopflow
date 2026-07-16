# Resolve Session bodies through the current Home lf

## Problem

A durable Project or Task Session pins its `lf` binary, database path, and `LF_HOME` at creation time in `ChildExecutionContext`. When that Session is later resumed — after an `lf` upgrade, a reinstall, or a binary move — the launch path reads the *historical* `ChildExecutionContext` and tries to execute the old binary. If the old path is gone or incompatible with the current database migration, the Session is stranded: its worktree, provider history, commands, and generation sequence are all intact in the store, but no process can be launched to continue them.

This is the exact failure behind W2-177, W2-178, W2-218, and W2-224: their stored `lf_bin` paths point to binaries that either no longer exist or reject the shared database migration `0.11.016_task_linear_observations`.

## The demo

A Task Session was created under `lf` v0.11.5. After upgrading to v0.12.0, `lf task resume W2-225` launches the new binary against the same worktree, provider history, and directive sequence. The `lf status` output shows the generation's provenance: "binary: v0.12.0 / release". The old v0.11.5 binary is gone from disk; the Session does not care.

## Approach

**Resolve at the launch boundary. Stop persisting for selection.**

At every point where a Session is launched (resume, supervisor wake, handoff completion), resolve the current `lf`, current store, and current `LF_HOME` from the running process's environment — the same way `pinned_execution_context()` does at creation time. Record what actually ran as immutable provenance on each `ChildProcessGeneration`. The persisted `ChildExecutionContext` columns (`lf_bin`, `db_path`, `lf_home`) are no longer read for launch; they stay in the schema as an audit trail until an earned table rebuild removes them.

### Changes

#### 1. `ChildProcessGeneration` gains binary provenance

Add three fields to `ChildProcessGeneration` in `child_session.rs`:

```rust
pub struct ChildProcessGeneration {
    // ... existing fields ...
    /// Build version of the lf binary that launched this generation.
    pub build_version: String,
    /// Provenance of the lf binary (development / release).
    pub build_provenance: String,
    /// Source identity of the lf binary (e.g. "loopflow-a1b2c3" or "release").
    pub build_source_identity: String,
}
```

These fields are immutable once recorded: they describe what ran, never what runs next.

A `BinaryProvenance` helper struct captures the three values:

```rust
pub struct BinaryProvenance {
    pub version: String,
    pub provenance: String,
    pub source_identity: String,
}

impl BinaryProvenance {
    pub fn current() -> Self {
        Self {
            version: env!("CARGO_PKG_VERSION").to_string(),
            provenance: crate::build_info::provenance().to_string(),
            source_identity: crate::build_info::source_identity(),
        }
    }
}
```

#### 2. Launch path resolves current Home, not persisted Session context

In `ops/project.rs::launch_project_process` and `ops/task.rs::launch_task_process`:

**Before (pseudocode):**
```rust
let execution = session.execution.clone().ok_or_else(|| ...)?;
// use execution.lf_bin, execution.db_path, execution.lf_home
```

**After:**
```rust
let execution = crate::engine::process::pinned_execution_context()
    .map_err(|error| /* "cannot resolve current lf: {error}" */)?;
let provenance = crate::child_session::BinaryProvenance::current();
// use execution.lf_bin, execution.db_path, execution.lf_home
// record provenance on the generation
```

The Session's `execution` field is no longer read at launch time. It remains written on new sessions for backwards compatibility with any external consumer that reads it.

#### 3. Validate before reserving a generation

Before calling `store.reserve_*_process()`, verify the resolved binary exists:

```rust
let lf_bin = crate::engine::process::resolve_pinned_lf_binary()
    .map_err(|error| task_error(format!(
        "cannot resolve current lf binary: {error}"
    )))?;
```

This fails visibly without burning a generation reservation. The existing `resolve_pinned_lf_binary()` already checks that the binary exists on disk and is absolute.

#### 4. Store `ChildProcessGeneration` provenance

Add a `process_provenance_json` TEXT column to both `task_sessions` and `project_sessions` tables. Store the `BinaryProvenance` as JSON:

```json
{
  "version": "0.11.0",
  "provenance": "release",
  "source_identity": "release"
}
```

**Migration:** `0.11.018_session_body_provenance.sql`

```sql
ALTER TABLE task_sessions ADD COLUMN process_provenance_json TEXT;
ALTER TABLE project_sessions ADD COLUMN process_provenance_json TEXT;
```

Update all SQL that reads/writes `ChildProcessGeneration` to include the new column:
- `TASK_SESSION_INSERT`, `TASK_SESSION_COLUMNS`, `TASK_SESSION_SELECT`, `TASK_SESSION_UPDATE`, `TASK_SESSION_LEASE_UPDATE`
- `PROJECT_SESSION_INSERT`, `PROJECT_SESSION_COLUMNS`, `PROJECT_SESSION_SELECT`, `PROJECT_SESSION_UPDATE`, `PROJECT_SESSION_LEASE_UPDATE`
- `reserve_task_process`, `reserve_project_process`
- `map_task_session_row`, `map_project_session_row`
- `task_session_control_params`, `project_session_control_params`

#### 5. Deprecate `ChildExecutionContext` for launch

The `ChildExecutionContext` struct and `execution` field on both session types remain. The Session's `execution` is still written on creation (for backwards compat and audit), but the launch path no longer reads it. The comment on `ChildExecutionContext` changes:

```rust
/// ~~The executable and store a Session runs against, pinned once when the
/// Session is created.~~
///
/// DEPRECATED for launch: the current Home lf is resolved at the launch
/// boundary, not read from the Session. Retained for audit: each Session
/// records which lf created it. Historical columns may remain until an
/// earned table rebuild.
```

#### 6. Status DTO provenance

The `format_child_body` function in `bin/lf.rs` already shows generation number, agent, and provider. Extend it to show provenance:

```
body: generation 3; agent claude; provider claude; binary v0.11.0 (release)
```

The `--json` output for session snapshots includes the full `ChildProcessGeneration`, which now carries provenance.

### File map

| File | Change |
|------|--------|
| `rust/loopflow/src/child_session.rs` | Add `BinaryProvenance`, add provenance fields to `ChildProcessGeneration`, update `for_tests()` |
| `rust/loopflow/src/engine/process.rs` | No change (pinned_execution_context already exists) |
| `rust/loopflow/src/ops/project.rs` | Replace `session.execution.clone()` with `pinned_execution_context()`, record provenance |
| `rust/loopflow/src/ops/task.rs` | Same: replace `session.execution.clone()` with `pinned_execution_context()`, record provenance |
| `rust/loopflow/src/store/migrations.rs` | Add migration 0.11.017 |
| `rust/loopflow/src/store/migrations/0.11.018_session_body_provenance.sql` | New: ALTER TABLE ADD COLUMN |
| `rust/loopflow/src/store/sqlite/child_sessions.rs` | Add `process_provenance_json` to all SQL, update row mappers and param builders |
| `rust/loopflow/src/bin/lf.rs` | Show provenance in `format_child_body` |
| `rust/loopflow/tests/support/mod.rs` | Update `ChildProcessGeneration` construction in test helpers |
| All test files constructing `ChildProcessGeneration` | Add provenance fields |

### De-risking

| Question | Finding | Impact on design |
|----------|---------|-----------------|
| Does the handoff completion wake a parent through the same launch path? | Yes. `handoff.rs::resume_task_parent` calls `resume_task_async` → `queue_command` → `session.launch()` → `launch_task_process`. Same fix covers handoff. | No additional change needed. |
| Can the resolved `lf` binary differ from the Session's `db_path`? | The resolved binary uses the current process's `db_path`. Since the parent already validated the Session exists in its store, and SQLite WAL provides consistent reads, the child will find the same Session row. | Safe. The parent validates before launch. |
| Does `extend_session_control_context` need changes? | No. It already checks `if !child_env.iter().any(...)` before setting CONTROL_* values. The resolved `pinned_execution_context` values are set as env vars before the function runs, so the function correctly uses them. | No change needed. |
| What about tests that construct `ChildProcessGeneration::for_tests()`? | Need updating to include provenance fields. The `for_tests()` constructor already exists and is used in ~20 places. | Update the constructor; all call sites get provenance for free. |
| Does `ChildExecutionContext` need a migration to drop columns? | No. The directive says "historical SQLite columns may remain until an earned table rebuild." Columns stay; they just stop being read for launch. | No migration for column removal. |
| Are there other consumers of `session.execution` besides launch? | Only the two launch functions and the store's write/read path. No other code reads `execution.lf_bin` etc. | Clean boundary. |

### Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Keep reading `session.execution` at launch but update it in-place on resume | Each resume overwrites the Session row with the current binary. Loses the audit trail of which binary originally created the Session. Also requires a write to the Session row before reserving a generation, creating a window for races. | Violates "provenance says what ran; it never selects what runs next." |
| Store provenance on `ChildProcessGeneration` as three separate TEXT columns | More queryable but three extra columns per table, more migration surface, more JOINs for status. | JSON is simpler, the provenance is a single audit blob, and no query filters on it. |
| Drop `ChildExecutionContext` columns in this migration | Saves schema space but requires a table rebuild and breaks any external tool reading the columns. | The directive explicitly says "historical SQLite columns may remain until an earned table rebuild." |
| Resolve binary lazily (at child boot time, not parent launch time) | The child would resolve its own `lf` binary when it starts. | The child is launched via tmux with explicit `argv[0]` and env vars. Changing this would require a different launch protocol. The parent resolving at launch is simpler and matches the existing pattern. |

#### 7. `begin_generation` stays session-owned, provenance is set by the launcher

The `begin_generation` method on `TaskSession` and `ProjectSession` creates the `ChildProcessGeneration` struct. It does not know which binary will launch this generation. The launcher (in `launch_task_process` / `launch_project_process`) calls `begin_generation`, then sets the provenance fields on the returned generation before the store writes it:

```rust
let generation = launch.begin_generation(tmux_name.clone());
if let Some(process) = &mut launch.latest_process {
    process.build_version = provenance.version.clone();
    process.build_provenance = provenance.provenance.clone();
    process.build_source_identity = provenance.source_identity.clone();
}
```

This keeps session logic (generation numbering, tmux naming) separate from launch provenance (which binary). Test helpers that call `begin_generation` get default/empty provenance, which is fine for unit tests that don't exercise the launch path.

### Key decisions

1. **Resolve at parent launch time, not child boot time.** The parent is the process that holds the Session and validates its state. It resolves the current binary and passes it to the child via argv and env. This matches the existing protocol.

2. **Provenance as JSON on the generation, not on the Session.** Each generation records what binary launched it. The Session's `execution` field records what binary created it. These are different things.

3. **Stop reading `execution` at launch, but keep writing it.** The Session still records which binary created it (for audit). The launch path resolves fresh. No backward compatibility shim.

4. **Validate before reserving.** The resolved binary is checked for existence before calling `reserve_*_process()`. A missing binary fails visibly without burning a generation.

### Scope

- In scope: Project Session launch, Task Session launch, handoff completion wake, binary provenance on generations, status DTO provenance, store migration
- Out of scope: Dropping `ChildExecutionContext` columns, redesigning the migration system, daemon dependencies, new Session successors, changing the child boot protocol

### Done when

1. `cargo test` passes (all existing tests updated, new tests pass)
2. A Session created under binary A, with binary A removed from disk, can be resumed through binary B with its worktree, provider history, directives, commands, and generation sequence intact
3. `lf status` shows binary provenance on active generations
4. The stranded sessions W2-177, W2-178, W2-218, W2-224 can be resumed (verify with `lf task resume <id>`)

### Proof (test plan)

1. **Unit test: provenance round-trip.** Create a `ChildProcessGeneration` with provenance, serialize/deserialize through the store, verify all three provenance fields survive.

2. **Integration test: resume through different binary.**
   - Create a Task Session under binary A (`/path/to/lf-a`)
   - Write the Session to the store
   - Remove binary A from disk
   - Update the current process's resolved binary to binary B (`/path/to/lf-b`)
   - Call `launch_task_process`
   - Verify: the generation's `lf` argv uses binary B, the provenance records binary B's build info, the Session's worktree/directives/provider history are unchanged

3. **Integration test: resume through different binary for Project Sessions.** Same as above but for Project Sessions.

4. **Integration test: handoff completion uses current binary.** Create a Task Session with a handoff pending, resolve a different binary, complete the handoff, verify the parent resumes through the current binary.

5. **Integration test: missing binary fails before reservation.** Set the resolved binary to a non-existent path, call `launch_task_process`, verify the error message names the missing binary and no generation was reserved.

6. **Status DTO test.** Create a session with provenance, call `format_child_body`, verify the output includes provenance info.

### Measure

Before: W2-177, W2-178, W2-218, W2-224 are stranded (resume fails with "lf binary does not exist" or migration rejection).

After: All four sessions resume successfully. The generation's provenance records the current binary's build info. `lf status` shows the provenance.
