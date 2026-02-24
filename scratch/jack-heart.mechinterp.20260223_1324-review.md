# Annotation Layer — Design Review

## What was implemented

A trace-first annotation system that records one versioned envelope per agent run as a sidecar file, then appends outcome signals after execution. Env vars propagate trace identity to all agent subprocesses regardless of backend.

**Core module**: `rust/loopflow/src/engine/annotation.rs` (473 lines)
- `AnnotationEnvelopeV1` schema with spawn metadata + optional outcome
- `TraceContext` with trace_id, span_id, parent_span_id (OpenTelemetry pattern)
- Sidecar lifecycle: `write_envelope` → agent runs → `append_outcome`
- `write_sidecar` convenience that handles gitignore and returns `AnnotationContext`
- `annotation_env_pairs` for propagating `LF_TRACE_ID`, `LF_SPAN_ID`, `LF_ANNOTATION_FILE`, `LF_STEP_TYPE`, `LF_FLOW_POSITION` to subprocesses
- 7 unit tests covering roundtrip, outcome append, env pairs, gitignore idempotency, and spawn metadata preservation

**Integration across all launch paths**:
- `lf` local runs (`run.rs`): builds envelope pre-launch, appends outcome post-launch
- Wave step runs (`wave/mod.rs`): writes sidecar with flow position before each step agent
- Fork branch runs (`wave/fork.rs`): writes per-branch sidecar in fork worktrees
- Docker executor (`docker.rs`): propagates annotation env vars into container env
- Local process executor (`local.rs`): propagates annotation env vars to child process

## Key choices

| Decision | Rationale |
|----------|-----------|
| Sidecar files over env-only | Researchers need durable records that survive process exit |
| `.lf/annotation/<trace_id>/envelope.json` layout | One directory per trace enables future sibling files (prompt_context.json) |
| Env var propagation (not prompt injection) | Model-agnostic, doesn't contaminate prompt or confound research |
| `schema_version: 1` from day one | Collaboration will evolve; versioning prevents breaking rewrites |
| Non-fatal sidecar writes | Annotation failure should never block agent execution |
| `WaveEnvelopeParams` struct for wave builder | Avoids 9-parameter function; clear field names |

## How it fits together

```
lf run / wave executor / fork executor
    │
    ├─ build_lf_envelope() or build_wave_envelope()
    ├─ write_sidecar()  →  .lf/annotation/<trace_id>/envelope.json
    ├─ annotation_env_pairs()  →  LF_TRACE_ID, LF_SPAN_ID, etc.
    │
    ├─ launch agent subprocess (env vars set)
    │
    └─ append_outcome()  →  merges exit_code, duration_ms, artifacts into envelope
```

The `AnnotationContext` struct carries the envelope + path through the launch pipeline. Each executor (`local.rs`, `docker.rs`) reads env pairs from it and sets them on the subprocess.

## Risks and bottlenecks

- **Sidecar write is not atomic**: read-modify-write in `append_outcome` has a TOCTOU window. Acceptable for single-writer-per-trace (current design), but would break if multiple processes wrote to the same envelope.
- **`git diff --name-only HEAD`** for artifact detection may miss committed changes in daemon workflows where agents commit before outcome append. Worth revisiting for V2.
- **`parent_span_id` is only populated for nested `lf` calls** (via `LF_SPAN_ID` env var inheritance). Wave-to-fork parent-child linkage is not wired yet — fork branches don't reference the parent step's span. This is a known V2 gap.
- **`.lf/annotation/` grows unbounded**. No cleanup/rotation mechanism yet. Should be addressed before heavy production use.

## What's not included

- **Provider adapters**: no Anthropic API header passthrough or Claude Code hook integration (design doc section 4, explicitly deferred to post-V1)
- **prompt_context.json**: the second sidecar file (doc list, diff tier, token breakdown) mentioned in the design doc is not implemented
- **Dashboards/analytics**: explicitly out of scope per wave non-goals
- **Trace ID correlation with Anthropic internal logs**: open research question
- **Sidecar cleanup**: no TTL, rotation, or garbage collection
