# Annotation Layer

## Problem

Loopflow currently emits rich prompts but loses the workflow labels that matter for mechanistic interpretability. A researcher can inspect API traffic, but cannot reliably answer: *what step was this, in what flow position, with what directives, and what happened after?*

Who benefits:
- **Mech interp collaborators** get labeled experiments instead of anonymous requests.
- **Loopflow maintainers** get reproducible, queryable traces of agent behavior across steps.

Why now:
- `wave/mechinterp/README.md` sets this as a core wave goal for 2026 collaboration work.
- Without instrumentation now, we cannot backfill trustworthy workflow metadata later.

## Approach

Build a **trace-first annotation system** that records one canonical envelope per agent run, then appends outcome signals after execution.

### 1) Define a versioned envelope schema

Create `AnnotationEnvelopeV1` with two phases:
- **Spawn fields** (known pre-run): `step`, `flow`, `direction`, `area`, `context`, `wave`, `trace`.
- **Outcome fields** (known post-run): `exit_code`, `verdict`, `tests`, `duration_ms`, `turns`, `artifacts_produced`.

Core identity fields:
- `trace.trace_id` (stable across one step run)
- `trace.span_id` (unique per agent launch)
- `trace.parent_span_id` (set for flow/fork relationships)

### 2) Write sidecar files for every launch path

Before launching an agent, write:
- `.lf/annotation/<trace_id>/envelope.json`
- `.lf/annotation/<trace_id>/prompt_context.json` (doc list, diff tier, token breakdown)

Cover both launch paths:
- `lf` local runs (`rust/loopflow/src/lf/commands/run.rs`)
- daemon wave runs (`rust/loopflow/src/lfd/executor/helpers.rs`, `.../wave/mod.rs`, `.../wave/fork.rs`)

### 3) Propagate metadata to the spawned agent process

Set env vars on every agent subprocess:
- `LF_TRACE_ID`
- `LF_SPAN_ID`
- `LF_ANNOTATION_FILE`
- `LF_STEP_TYPE`
- `LF_FLOW_POSITION`

This keeps propagation model-agnostic and works for Claude/Codex/Gemini/OpenCode immediately.

### 4) Add provider adapters without blocking V1

V1 ships with sidecar+env only. Then add adapters:
- **Anthropic direct API calls**: include metadata header/body field when available.
- **Claude Code hook**: read `LF_ANNOTATION_FILE` and attach metadata to outgoing requests.

If provider passthrough is unavailable, sidecar data remains the source of truth.

### 5) Append outcomes atomically

After agent exit, update `envelope.json` with outcome fields:
- `exit_code` and `duration_ms` from runner
- `artifacts_produced` from `git diff --name-only`
- `verdict` parsed only for gate-style steps (else `null`)

Never overwrite spawn metadata; only append outcome section.

### Research patterns this follows

- **OpenTelemetry pattern**: trace/span identity + structured attributes.
- **Event-sourcing pattern**: immutable pre-run record, append-only post-run outcome.
- **Observability fallback pattern**: capture locally first, integrate remote joins later.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Prompt-inject metadata block on every request | Highest visibility, but contaminates model behavior and confounds research | We need inert instrumentation, not extra instructions in the prompt |
| Env vars only, no sidecar | Easy to implement, but metadata disappears once process exits | Researchers need durable records keyed to outcomes |
| Full OTEL collector/exporter first | Powerful long-term analytics | Too heavy for wave scope; violates "instrument first" priority |

## Key decisions

- **Decision: ship sidecar + env propagation as the hard requirement for V1.**
  - Following wave goal: **"Build the annotation layer that makes collaboration valuable. Every LLM call carries structured metadata about its workflow context."**
  - Rationale: this is the minimum implementation that is both durable and provider-agnostic.

- **Decision: treat dashboards/analytics as out of scope.**
  - Following wave non-goal: **"Not building analytics or dashboards."**
  - Rationale: we are producing clean labeled data, not a product surface.

- **Decision: avoid synthetic experiments in this phase.**
  - Following wave non-goal: **"Not generating synthetic experiments with open-weight models."**
  - Rationale: prioritize real workflow traces from actual loopflow runs.

- **Decision: use versioned schema from day one (`schema_version: 1`).**
  - Rationale: collaboration will evolve quickly; versioning prevents breaking rewrites.

### Wild success (6 months)

Researchers can join loopflow run traces with internal activation slices and answer one concrete question (for example, gate overconfidence) with labeled evidence in days, not weeks.

### Wild failure (6 months)

Instrumentation exists but is unusable: fields are inconsistent across run paths, no stable trace IDs, and outcome linkage is missing. Mitigation: one shared schema type + contract tests across both `lf` and `lfd` launch paths.

## Scope

- In scope:
  - Versioned annotation schema
  - Sidecar file write/read lifecycle
  - Env var propagation for all agent subprocesses
  - Outcome append on completion
  - Tests that prove parity across local runs, wave runs, and fork runs

- Out of scope:
  - Dashboards, query UI, or metrics productization
  - Anthropic-internal data warehouse integration work
  - Model-behavior claims from this branch (instrumentation only)
  - Backfill of historical runs before annotation exists

## Done when

- `cargo test -p loopflow annotation_` passes with new schema + lifecycle tests.
- Running `lf implement -b "noop"` writes `.lf/annotation/<trace_id>/envelope.json` with spawn metadata and appended outcome.
- Running a wave step and a forked flow step writes envelopes with correct `flow.position` and parent/child trace linkage.
- For all supported agent backends, subprocess env includes `LF_TRACE_ID` and `LF_ANNOTATION_FILE`.
