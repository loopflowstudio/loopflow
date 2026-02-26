# Listen Authoring

## Problem

Listen stimuli exist in the data model (Phase 01) but can only be created via the HTTP API. Users declaring waves in schema YAML files — `wave/<name>/<name>.yaml` — cannot use listen stimuli. This forces a split workflow: YAML for flow/area/direction, API calls for listen wiring.

Worse, listen stimuli don't actually fire. The executor marks a run complete and goes idle. No code checks for listening waves when a source completes. Listen stimuli are inert data.

This blocks chord-based workflows where one wave reacts to another's output.

## Approach

Three deliverables, in order:

### 1. Schema YAML support for listen stimuli

Add `source` and optional `source_repo` fields to `StimulusDef`. Extend `parse_stimulus` to handle `kind: listen`. Resolve the source wave name to an ID during `create_wave_handler`, scoped by repo.

```yaml
# wave/designer/designer.yaml
flow: build
area: [designs/]
direction: [ux]
stimulus:
  kind: listen
  source: infra    # wave name — resolved to ID at creation time
  # optional; defaults to current wave repo
  source_repo: /Users/jack/src/other-repo
```

**Changes:**

- `wave_config.rs`: Add `source: Option<String>` and `source_repo: Option<String>` to `StimulusDef`
- `waves.rs`: Change `parse_stimulus` to return a `ParsedStimulus` struct (kind, cron, source, source_repo) instead of a tuple. Handle `"listen"` in the match, requiring `source` and rejecting `cron`. In `create_wave_handler`, resolve `source` name → `source_wave_id` via a repo-scoped resolver (`source_repo` if set, else current wave repo), validate it's not a self-reference, and wire it into the `Stimulus`.

**Constraint:** Source wave must exist at creation time in the selected source repo. Create waves in dependency order (source first, listener second). This matches the HTTP API's existing behavior and avoids deferred resolution complexity.

### 2. Listen trigger on wave completion

When a wave run completes (`FlowAction::Complete` in `executor/wave/mod.rs`), find all enabled listen stimuli that reference this wave as their source and start those listening waves.

**Changes:**

- `executor/wave/mod.rs` in the `FlowAction::Complete` arm, after `update_wave_run` and before `return Ok(())`: query `list_stimuli_by_kind(Listen)`, filter to `source_wave_id == completing_wave_id`, and for each listening wave:
  - start immediately via internal trigger/executor path (`create_wave_run_with_id` + scheduler slot + `spawn_run_task_with_slot`) when possible, or
  - queue/coalesce a pending activation when the listener is already running or scheduler capacity is full.
- Use existing `list_stimuli_by_kind(5)` + in-memory filter rather than adding a new store query. Listen stimuli will be few (single digits per deployment).
- Add a pending-activation drain loop (ticker) that retries queued activations across all stimulus kinds when waves become runnable. This prevents dropped triggers when scheduler capacity is temporarily saturated.

**Trigger semantics:**
- Only fires on `Completed`, not `Failed` — a failing source shouldn't cascade.
- The listening wave starts a normal run with no special context (context injection is designed separately).
- If the listening wave is already running, queue/coalesce one pending activation per `(wave_id, stimulus_id)`.
- If listener wave is `Paused`, skip it.
- Update `last_triggered_at` when the listen trigger is accepted (run started or activation queued).

### 3. Sidecar → CI fix terminology rename

The `sidecar.rs` module and `SidecarKind` type predate the listen stimulus. They implement CI fix agents, not listening flows. Rename to match what the code actually does:

| Before | After |
|--------|-------|
| `SidecarKind` | `CiFixKind` |
| `sidecar_kind` field on `WaveRun` | `ci_fix_kind` |
| `executor/wave/sidecar.rs` | `executor/wave/ci_fix.rs` |
| `WaveRunKind::Sidecar` | `WaveRunKind::CiFix` |
| `sidecar_kind` SQL column | `ci_fix_kind` (new migration, not 014) |
| `docs/lfd.md` "CI sidecar agents" | "CI fix agents" |

`014_rename_provider_to_harness.sql` already exists. Add a new migration (next number, e.g. 015) to rename `sidecar_kind` → `ci_fix_kind`. The column already stores `CiFix = 1` — this is a column rename, no data change.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Lazy source resolution (store name, resolve at trigger time) | Tolerates out-of-order wave creation | Adds complexity. `source_wave_id` is an FK — storing a name would require a second field or dropping the FK. Create waves in the right order instead. |
| Disallow cross-repo listen | Simpler resolver | Too restrictive. Chords often coordinate work across repos. Allow cross-repo by explicit `source_repo`, defaulting to current repo for easy UX. |
| Dedicated `list_stimuli_by_source` query | Cleaner than filtering in memory | Over-engineering for single-digit listen stimuli counts. Add it later if needed. |
| Queue only for listen, keep other kinds lossy at scheduler saturation | Smaller patch | Inconsistent reliability model. If queueing is the goal, apply it uniformly via shared pending-activation drain behavior. |
| Context injection in this phase | Listening wave knows what its source did | Doubles the scope. Schema support + triggering is already a complete milestone. Design context injection, implement in Phase 04. |
| Rename sidecar → listening | Original plan from wave item | Wrong mapping. Sidecars are CI fix agents, not listeners. Renaming to "listening" would be misleading. Rename to what it is: ci_fix. |
| Trigger on Failed too | Listener can react to failures | Cascading failures are painful. Start with success-only triggers. Users can add failure triggering later. |

## Key decisions

**Source resolution is eager and repo-scoped.** The source wave must exist when the listener is created, in `source_repo` if specified, otherwise in the listener's repo. This keeps the FK constraint and avoids "source not found at trigger time" errors. Tradeoff: waves must still be created in dependency order.

**Trigger fires from the executor, not from events or HTTP helpers.** The completion path in `execute()` directly queries for listen stimuli and starts listening waves through internal run-creation helpers. No event bus or route indirection.

**Queued activations are first-class, not best-effort drops.** If a trigger cannot start immediately (listener running or scheduler full), persist/coalesce it in `pending_activations` and let a drain loop retry until it starts.

**SidecarKind stays as an enum (renamed to CiFixKind), not collapsed to a bool.** There's only one variant today, but the enum is extensible and the SQL column already stores an integer. Collapsing to bool would require changing the persistence layer for no functional gain.

**Context injection is designed but not built.** It's a separate concern: assembling source run metadata (PR, branch, files changed) into the listening wave's prompt context. Design it here, build it in Phase 04 when we can validate the trigger mechanism works in practice.

## Context injection design (Phase 04)

When a listen stimulus fires, the executor could assemble context from the source wave's completed run metadata:

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

For cross-repo listening, context assembly should use persisted run/PR metadata APIs; do not grant the listening executor arbitrary filesystem access to source repos.

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
- Optional `source_repo` on listen stimuli (defaulting to current repo)
- Listen trigger on source wave completion (start listening wave)
- Pending activation queueing/coalescing for listen triggers and scheduler-capacity retries
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

# Listen queueing/coalescing drains pending activation
cargo test -p loopflow -- listen_queue

# Sidecar renamed to ci_fix
cargo test -p loopflow --all   # no sidecar references in code
grep -r "sidecar" rust/loopflow/src/ --include="*.rs" | grep -v "ci_fix" # should be empty

# Full suite passes
cargo fmt --check && cargo clippy -- -D warnings && cargo test --all
uv run pytest python/tests/
```
