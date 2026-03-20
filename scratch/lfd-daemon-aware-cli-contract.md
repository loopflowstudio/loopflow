---
linear_id: b00f983a-47b2-4ea8-b357-e45e0d183aa3
---
# Runtime Journal Protocol v2

## Protocol

Every event is a single JSON line with two discriminator fields:

```json
{ "run_id": "...", "ts": "...", "node": "run|flow|step", "event": "started|completed|errored|escalated", ...fields }
```

### Node × event matrix

| node | event | extra fields |
|------|-------|--------------|
| run | started | wave_name, worktree, command |
| run | completed | — |
| run | errored | error |
| run | escalated | signal |
| flow | started | flow |
| flow | completed | — |
| flow | errored | error |
| flow | escalated | signal |
| step | started | step, index |
| step | completed | step, index |
| step | errored | step, index, error |
| step | escalated | step, index, signal |

### Happy path sequence

```
{ node: "run",  event: "started",   run_id, wave_name, worktree, command }
  { node: "flow", event: "started",   run_id, flow }
    { node: "step", event: "started",   run_id, step, index: 0 }
    { node: "step", event: "completed", run_id, step, index: 0 }
    { node: "step", event: "started",   run_id, step, index: 1 }
    { node: "step", event: "completed", run_id, step, index: 1 }
  { node: "flow", event: "completed", run_id }
{ node: "run",  event: "completed", run_id }
```

### Single-step runs (no flow)

```
{ node: "run",  event: "started",   run_id, wave_name, worktree, command }
  { node: "step", event: "started",   run_id, step, index: 0 }
  { node: "step", event: "completed", run_id, step, index: 0 }
{ node: "run",  event: "completed", run_id }
```

No flow bracket — the step nests directly under the run.

### Error path

When a step fails to run (missing prompt, agent crash):

```
    { node: "step", event: "errored", run_id, step, index, error: "..." }
  { node: "flow", event: "errored", run_id, error: "step implement failed" }
{ node: "run",  event: "errored", run_id, error: "..." }
```

Errors propagate up — a step error causes flow error causes run error.

### Escalation

Agent flags something for human attention:

```
    { node: "step", event: "escalated", run_id, step, index, signal: "..." }
  { node: "flow", event: "escalated", run_id, signal: "..." }
{ node: "run",  event: "escalated", run_id, signal: "..." }
```

## Changes from v1

- **`flow.started` / `flow.completed` are new** — run and flow are separate brackets. Previously flow identity was crammed into `run.started` as `flow?`.
- **`run.waiting` removed** — interactive steps are implicit from flow definition. Consumer knows which steps are interactive.
- **`exit_code` removed from completed events** — completed means success. Failure is `errored`.
- **`escalated` is new** — third terminal state for agent-initiated escalation.
- **Single event struct** — `node` + `event` discriminators replace separate Rust enum variants per event type.
- **Timestamps preserved through replay** — lfd must use the journal timestamp, not regenerate on read.

## Implementation

### Rust types

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LfNode {
    Run,
    Flow,
    Step,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LfEventType {
    Started,
    Completed,
    Errored,
    Escalated,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LfEvent {
    pub run_id: LfdId,
    #[serde(with = "time::serde::rfc3339")]
    pub ts: OffsetDateTime,
    pub node: LfNode,
    pub event: LfEventType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wave_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub worktree: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flow: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub step: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub index: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signal: Option<String>,
}
```

### Emitting

Drop `RuntimeRun` object. Replace with a free function in `journal/`:

```rust
journal::emit(repo, node, event, fields)
```

No `Option<&RuntimeRun>` threading through flow.rs. The emit function checks if we're in a wave worktree and writes or silently no-ops.

### Daemon-side replay

`LfObserver` replaces `RuntimeJournalObserver`. `map_runtime_event` is deleted — `LfEvent` maps 1:1 to the websocket `Event` variants. Preserve the original `ts`.

### What to delete

- `RuntimeRun` struct and its `Cell<bool>` finished guard
- `RuntimeRunMeta` / `meta.json` — the `run.started` event carries the same info
- `RuntimeEvent` enum — replaced by `LfEvent`
- `RunTarget` enum — no longer needed, flow identity comes from `flow.started`
- `map_runtime_event` — no translation needed
- All `Option<&RuntimeRun>` parameters in flow.rs and lf.rs
- `runtime/mod.rs` → replaced by `journal/mod.rs`
- `lfd/runtime_journal.rs` → replaced by `lfd/journal.rs`
- `.lf/runtime/` → replaced by `.lf/journal/`
