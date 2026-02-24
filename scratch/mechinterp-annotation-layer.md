# Annotation Layer — V1

Trace-first annotation system. Records one versioned envelope per agent run as a sidecar file, appends outcome signals after execution. Env vars propagate trace identity to all agent subprocesses regardless of backend.

## What shipped

**Core module**: `rust/loopflow/src/engine/annotation.rs`
- `AnnotationEnvelopeV1` schema with spawn metadata + optional outcome
- `TraceContext` with trace_id, span_id, parent_span_id (OpenTelemetry pattern)
- Sidecar lifecycle: `write_envelope` → agent runs → `append_outcome`
- `annotation_env_pairs` propagates `LF_TRACE_ID`, `LF_SPAN_ID`, `LF_ANNOTATION_FILE`, `LF_STEP_TYPE`, `LF_FLOW_POSITION`
- 7 unit tests covering roundtrip, outcome append, env pairs, gitignore idempotency, spawn metadata preservation

**All launch paths instrumented**:
- `lf` local runs (`run.rs`): envelope pre-launch, outcome post-launch
- Wave step runs (`wave/mod.rs`): sidecar with flow position before each step
- Fork branch runs (`wave/fork.rs`): per-branch sidecar in fork worktrees
- Docker executor (`docker.rs`): annotation env vars in container env
- Local process executor (`local.rs`): annotation env vars on child process

**Data flow**:
```
lf run / wave executor / fork executor
    ├─ build_lf_envelope() or build_wave_envelope()
    ├─ write_sidecar()  →  .lf/annotation/<trace_id>/envelope.json
    ├─ annotation_env_pairs()  →  LF_TRACE_ID, LF_SPAN_ID, etc.
    ├─ launch agent subprocess (env vars set)
    └─ append_outcome()  →  merges exit_code, duration_ms, artifacts
```

## Key decisions

| Decision | Rationale |
|----------|-----------|
| Sidecar files over env-only | Durable records that survive process exit |
| `.lf/annotation/<trace_id>/envelope.json` | One dir per trace, future sibling files (prompt_context.json) |
| Env var propagation, not prompt injection | Model-agnostic, doesn't contaminate prompt or confound research |
| `schema_version: 1` from day one | Collaboration evolves; versioning prevents breaking rewrites |
| Non-fatal sidecar writes | Annotation failure never blocks agent execution |
| `WaveEnvelopeParams` struct | Avoids 9-parameter function; clear field names |

Alternatives rejected: prompt-injected metadata (contaminates model behavior), env-only without sidecar (metadata lost on exit), full OTEL collector (too heavy for wave scope).

## Known risks

- **Non-atomic outcome append**: read-modify-write TOCTOU in `append_outcome`. Fine for single-writer-per-trace, breaks with concurrent writers.
- **Artifact detection via `git diff --name-only HEAD`**: misses committed changes when agents commit before outcome append.
- **Unbounded `.lf/annotation/` growth**: no cleanup/rotation yet.

## V2 gaps

- **Parent-child trace linkage for forks**: `parent_span_id` only populated for nested `lf` calls via env inheritance. Wave-to-fork linkage not wired.
- **`prompt_context.json`**: second sidecar file (doc list, diff tier, token breakdown) not implemented.
- **Provider adapters**: no Anthropic API header passthrough or Claude Code hook integration.
- **Sidecar cleanup**: no TTL, rotation, or garbage collection.
- **Trace ID correlation with Anthropic internal logs**: open research question.
