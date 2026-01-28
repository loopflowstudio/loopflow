# Rust Roadmap: Protocol-First Engine (Stage 1)

Define the stable API that all clients and services use to control Loopflow.

## Goal
Create a versioned, transport-agnostic protocol that supports local and remote control of Loopflow. This is the foundation for managed clusters, Concerto integration, and CLI clients.

## Scope
- Request/response schema and event stream
- Versioning and compatibility rules
- Authn/z and multi-tenant routing hooks
- Error model and retry semantics

## Non-goals
- Implementing the server runtime
- Migrating existing Python internals

## Key decisions
- **Transport:** gRPC over HTTP/2 for structured APIs; JSON over HTTP for simple tooling.
- **Streaming:** server-side event stream for run progress.
- **Versioning:** semantic protocol version with strict compatibility checks.
- **Error model:** typed errors with retry hints and idempotency keys.

## API surface (v1)
### Runs
- `CreateRun(flow_id, area, direction, params)`
- `CancelRun(run_id)`
- `GetRun(run_id)`
- `ListRuns(filters)`

### Flows
- `StartFlow(flow_id, area, direction, params)`
- `GetFlow(flow_id)`
- `ValidateFlow(flow_spec)`

### Events
- `ReportEvent(run_id, event)`
- `WatchEvents(filters)` (stream)

## Errors
- Typed error codes with retry hints (retryable, terminal, auth, not found).
- Include `trace_id` for observability.
- Idempotency keys for run creation and cancellation.

## Authn/z
- API keys for early phase.
- JWT/OIDC for enterprise integration.
- Tenant and project scoping in every request.

## Compatibility
- Protocol version in handshake and every response.
- Client must refuse incompatible versions.
- Server advertises supported version ranges.

## Typed RPC options: gRPC vs other schema‑first protocols
### gRPC + Protobuf (recommended default)
- **Pros:** strong schema, excellent codegen for mobile/desktop, efficient streaming, ecosystem maturity.
- **Cons:** requires HTTP/2, harder to debug with curl, protobuf schema language constraints.

### Connect (gRPC‑compatible over HTTP/1.1)
- **Pros:** Protobuf‑based with better browser compatibility; can speak gRPC or JSON.
- **Cons:** smaller ecosystem; still needs protobuf schemas and tooling.

### Twirp (RPC over HTTP/1.1 + Protobuf)
- **Pros:** simple deployment, protobuf schema, easy debugging.
- **Cons:** no native streaming; less standard for multi‑language clients.

### Thrift
- **Pros:** mature IDL, multiple transports.
- **Cons:** weaker modern tooling; less common on mobile; streaming story weaker.

### Cap’n Proto / FlatBuffers
- **Pros:** very fast, zero‑copy; great for high‑throughput.
- **Cons:** limited tooling and cross‑platform adoption; not ideal for public APIs.

### JSON Schema + REST
- **Pros:** maximum compatibility; easy tooling.
- **Cons:** weaker typing guarantees; harder to evolve safely; streaming is ad‑hoc.

### Practical stance
- **gRPC + Protobuf** for core control plane APIs and streaming events.
- Optionally **Connect** as a compatibility layer for browser/mobile clients.
- Avoid non‑protobuf IDLs unless a specific client forces it.

## Internal protocol (lf/lfd ↔ lfd‑core)
- Must be **well‑typed** and schema‑first.
- Treat as the primary design artifact; public surfaces derive from it.

## Remote lf behavior
- Remote `lf` talks to **`lfd`** (control plane), never directly to `lfd‑core`.
- Local `lf` must still work **without** a daemon running.
- Local mode uses direct `lf` ↔ `lfd-core` calls.
- `lfd` must expose the **subset of `lfd-core` APIs** that `lf` calls in local mode.
- This subset is the **engine contract**; remote mode is an engine swap.

### Engine contract (subset parity)
The engine contract is the minimal API surface that `lf` depends on in local mode. `lfd` must expose an equivalent remote API so `lf` can switch engines without changing UX.

**Core execution**
- `ExecuteFlow(flow_id, area, direction, params)` → run result + events
- `ExecuteStep(step_id, context, direction)` → step result + events
- `CancelExecution(run_id)`

**Context + prompt**
- `GatherContext(area, rules, diff_mode)` → context bundle
- `FormatPrompt(components)` → prompt text

**Flows + steps**
- `LoadFlow(flow_id)` → flow graph
- `ValidateFlow(flow_spec)` → errors
- `LoadStep(step_id)` → step definition

**Tokens + limits**
- `CountTokens(text, model)` → token count
- `EnforceLimits(context)` → trimmed/validated bundle

**Artifacts**
- `WriteArtifact(path, contents)`
- `ReadArtifact(path)` (if needed for resume)

**Events**
- `WatchEvents(filters)` (stream)

### gRPC proto sketch (engine contract)
```proto
syntax = "proto3";

package loopflow.engine.v1;

service Engine {
  // Core execution
  rpc ExecuteFlow(ExecuteFlowRequest) returns (ExecuteFlowResponse);
  rpc ExecuteStep(ExecuteStepRequest) returns (ExecuteStepResponse);
  rpc CancelExecution(CancelExecutionRequest) returns (CancelExecutionResponse);

  // Context + prompt
  rpc GatherContext(GatherContextRequest) returns (GatherContextResponse);
  rpc FormatPrompt(FormatPromptRequest) returns (FormatPromptResponse);

  // Flows + steps
  rpc LoadFlow(LoadFlowRequest) returns (LoadFlowResponse);
  rpc ValidateFlow(ValidateFlowRequest) returns (ValidateFlowResponse);
  rpc LoadStep(LoadStepRequest) returns (LoadStepResponse);

  // Tokens + limits
  rpc CountTokens(CountTokensRequest) returns (CountTokensResponse);
  rpc EnforceLimits(EnforceLimitsRequest) returns (EnforceLimitsResponse);

  // Artifacts
  rpc WriteArtifact(WriteArtifactRequest) returns (WriteArtifactResponse);
  rpc ReadArtifact(ReadArtifactRequest) returns (ReadArtifactResponse);

  // Events
  rpc WatchEvents(WatchEventsRequest) returns (stream Event);
}

message ExecuteFlowRequest {
  string flow_id = 1;
  string area = 2;
  repeated string direction = 3;
  map<string, string> params = 4;
}

message ExecuteFlowResponse {
  string run_id = 1;
  RunResult result = 2;
}

message ExecuteStepRequest {
  string step_id = 1;
  ContextBundle context = 2;
  repeated string direction = 3;
}

message ExecuteStepResponse {
  string run_id = 1;
  RunResult result = 2;
}

message CancelExecutionRequest {
  string run_id = 1;
}

message CancelExecutionResponse {
  bool cancelled = 1;
}

message GatherContextRequest {
  string area = 1;
  repeated string rules = 2;
  string diff_mode = 3;
}

message GatherContextResponse {
  ContextBundle context = 1;
}

message FormatPromptRequest {
  PromptComponents components = 1;
}

message FormatPromptResponse {
  string prompt = 1;
}

message LoadFlowRequest {
  string flow_id = 1;
}

message LoadFlowResponse {
  Flow flow = 1;
}

message ValidateFlowRequest {
  string flow_spec = 1;
}

message ValidateFlowResponse {
  repeated ValidationError errors = 1;
}

message LoadStepRequest {
  string step_id = 1;
}

message LoadStepResponse {
  Step step = 1;
}

message CountTokensRequest {
  string text = 1;
  string model = 2;
}

message CountTokensResponse {
  uint64 tokens = 1;
}

message EnforceLimitsRequest {
  ContextBundle context = 1;
}

message EnforceLimitsResponse {
  ContextBundle context = 1;
  repeated ValidationError warnings = 2;
}

message WriteArtifactRequest {
  string path = 1;
  bytes contents = 2;
}

message WriteArtifactResponse {
  bool ok = 1;
}

message ReadArtifactRequest {
  string path = 1;
}

message ReadArtifactResponse {
  bytes contents = 1;
}

message WatchEventsRequest {
  string run_id = 1;
}

message Event {
  string run_id = 1;
  string kind = 2;
  string message = 3;
  map<string, string> data = 4;
}
```

## UX compatibility requirements
- **Must not change:** prompt/flow semantics, artifact paths, CLI affordances.
- **Should not change:** direction composition, flow execution order, local defaults.
- **Ambiguous:** token counting heuristics, scheduling jitter, minor error strings.

## Success criteria
- `lf` can target local or remote `lfd` with no behavior differences.
- Concerto can drive runs over the same API.
- Backward-compatible changes are routine; breaking changes are rare and explicit.
- Protocol supports remote clients beyond desktop (mobile app readiness).
- Artifact and prompt paths remain unchanged.

## Open questions
- Do we want strict protobuf-only for v1 to avoid dual protocols?
- Do we need bi-directional streaming for interactive steps?
