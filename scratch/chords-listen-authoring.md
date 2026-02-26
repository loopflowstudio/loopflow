# Listen Authoring

## Problem

Listen stimuli exist in the data model (Phase 01) but can only be created via the HTTP API. Users declaring waves in schema YAML files — `wave/<name>/<name>.yaml` — cannot use listen stimuli. This forces a split workflow: YAML for flow/area/direction, API calls for listen wiring.

Worse, listen stimuli don't actually fire. The executor marks a run complete and goes idle. No code checks for listening waves when a source completes. Listen stimuli are inert data.

This blocks chord-based workflows where one wave reacts to another's output.

## Approach

Three deliverables, in order:

### 1. Schema YAML support for listen stimuli

Add `source` field to `StimulusDef`. Extend `parse_stimulus` to handle `kind: listen`. Resolve the source wave name to an ID during `create_wave_handler`.

```yaml
# wave/designer/designer.yaml
flow: build
area: [designs/]
direction: [ux]
stimulus:
  kind: listen
  source: infra    # wave name — resolved to ID at creation time
```

**Changes:**

- `wave_config.rs`: Add `source: Option<String>` to `StimulusDef`
- `waves.rs`: Change `parse_stimulus` to return a `ParsedStimulus` struct (kind, cron, source) instead of a tuple. Handle `"listen"` in the match, requiring `source` and rejecting `cron`. In `create_wave_handler`, resolve `source` name → `source_wave_id` via `resolve_wave_id`, validate it's not a self-reference, and wire it into the `Stimulus`.

**Constraint:** Source wave must exist at creation time. Create waves in dependency order (source first, listener second). This matches the HTTP API's existing behavior and avoids deferred resolution complexity.

### 2. Listen trigger on wave completion

When a wave run completes (`FlowAction::Complete` in `executor/wave/mod.rs`), find all enabled listen stimuli that reference this wave as their source and start those listening waves.

**Changes:**

- `executor/wave/mod.rs` in the `FlowAction::Complete` arm, after `update_wave_run` and before `return Ok(())`: query `list_stimuli_by_kind(Listen)`, filter to `source_wave_id == completing_wave_id`, and for each listening wave call `start_wave_run`.
- Use existing `list_stimuli_by_kind(5)` + in-memory filter rather than adding a new store query. Listen stimuli will be few (single digits per deployment).

**Trigger semantics:**
- Only fires on `Completed`, not `Failed` — a failing source shouldn't cascade.
- The listening wave starts a normal run with no special context (context injection is designed separately).
- If the listening wave is already running, skip it (don't queue duplicate runs).
- Update `last_triggered_at` on the listen stimulus after firing.

### 3. Sidecar → CI fix terminology rename

The `sidecar.rs` module and `SidecarKind` type predate the listen stimulus. They implement CI fix agents, not listening flows. Rename to match what the code actually does:

| Before | After |
|--------|-------|
| `SidecarKind` | `CiFixKind` |
| `sidecar_kind` field on `WaveRun` | `ci_fix_kind` |
| `executor/wave/sidecar.rs` | `executor/wave/ci_fix.rs` |
| `WaveRunKind::Sidecar` | `WaveRunKind::CiFix` |
| `sidecar_kind` SQL column | `ci_fix_kind` (migration 014) |
| `docs/lfd.md` "CI sidecar agents" | "CI fix agents" |

SQL migration 014 renames the column. The `sidecar_kind` column already stores `CiFix = 1` — the migration is a column rename, no data change.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Lazy source resolution (store name, resolve at trigger time) | Tolerates out-of-order wave creation | Adds complexity. `source_wave_id` is an FK — storing a name would require a second field or dropping the FK. Create waves in the right order instead. |
| Dedicated `list_stimuli_by_source` query | Cleaner than filtering in memory | Over-engineering for single-digit listen stimuli counts. Add it later if needed. |
| Context injection in this phase | Listening wave knows what its source did | Doubles the scope. Schema support + triggering is already a complete milestone. Design context injection, implement in Phase 04. |
| Rename sidecar → listening | Original plan from wave item | Wrong mapping. Sidecars are CI fix agents, not listeners. Renaming to "listening" would be misleading. Rename to what it is: ci_fix. |
| Trigger on Failed too | Listener can react to failures | Cascading failures are painful. Start with success-only triggers. Users can add failure triggering later. |

## Key decisions

**Source resolution is eager, not lazy.** The source wave must exist when the listener is created. This matches the HTTP API, keeps the FK constraint, and avoids a class of "source not found at trigger time" errors. Tradeoff: you must create waves in dependency order.

**Trigger fires from the executor, not from events.** The completion path in `execute()` directly queries for listen stimuli and starts listening waves. No event bus indirection. The executor already has store access and the wave ID. Adding an event handler would split the completion logic across two files for no benefit.

**SidecarKind stays as an enum (renamed to CiFixKind), not collapsed to a bool.** There's only one variant today, but the enum is extensible and the SQL column already stores an integer. Collapsing to bool would require changing the persistence layer for no functional gain.

**Context injection is designed but not built.** It's a separate concern: assembling source run metadata (PR, branch, files changed) into the listening wave's prompt context. Design it here, build it in Phase 04 when we can validate the trigger mechanism works in practice.

## Context injection design (Phase 04)

When a listen stimulus fires, the executor could assemble context from the source wave's completed run:

```rust
struct SourceContext {
    wave_name: String,
    pr_title: Option<String>,
    pr_url: Option<String>,
    branch: String,
    // Diff summary from the PR — not the full diff
    changed_files: Vec<String>,
}
```

This context would be injected as a clipboard message into the listening wave's first step, similar to how `lf debug -c` passes error context. The prompt would see: "Source wave `infra` completed. PR: 'Upgrade TLS library' (#142). Changed: `rust/loopflow/src/lfd/http/tls.rs`, `Cargo.toml`."

**Open question:** Should context depth be configurable per-stimulus? Three levels seem natural:
- `summary` (default): PR title + changed file list
- `full`: summary + full diff content
- `none`: just the trigger signal, no context

This belongs in `StimulusDef` as an optional `context` field:
```yaml
stimulus:
  kind: listen
  source: infra
  context: summary   # optional, default summary
```

## Scope

**In scope:**
- `listen` kind in wave schema YAML files with source name resolution
- Listen trigger on source wave completion (start listening wave)
- Sidecar → ci_fix rename across Rust code, SQL, docs
- Tests for schema parsing, trigger firing, and name resolution

**Out of scope:**
- Context injection implementation (designed above, Phase 04)
- Multi-source listen (listening to multiple waves — requires schema change to list)
- Listen on failure (trigger only on Completed for now)
- Trigger chaining safeguards (A listens to B listens to A — detect cycles)

## Done when

```bash
# Schema parsing accepts listen stimulus
cargo test -p loopflow -- listen_stimulus_schema

# Listen trigger fires on source completion
cargo test -p loopflow -- listen_trigger

# Sidecar renamed to ci_fix
cargo test -p loopflow --all   # no sidecar references in code
grep -r "sidecar" rust/loopflow/src/ --include="*.rs" | grep -v "ci_fix" # should be empty

# Full suite passes
cargo fmt --check && cargo clippy -- -D warnings && cargo test --all
uv run pytest python/tests/
```
