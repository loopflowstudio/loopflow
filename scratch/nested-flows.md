# Nested Flow Support

Design doc for supporting nested flow references and preserving source provenance through the execution stack.

## Problem

Flow definitions reference other flows (e.g., `grind` contains `ship`, `ship` contains `implement`). Currently:

1. **Rust lfd rejects nested flows** - `WaveExecutor` only accepts `FlowItem::Step` in fork branches (executor.rs:277-285, 318-325)
2. **Commit messages lose context** - `autocommit()` only knows the leaf step name, not the flow chain
3. **No flow references in Rust** - `FlowItem` enum has Step, Fork, Choose but no Flow variant
4. **Flat execution model** - `step_index` is a simple integer into `flow.items`

Goal: Maintain nested flow definitions for authoring while flattening to concrete steps for execution, preserving the flow parents for commit messages and UI.

## Proposed Changes

### 1. Fork Consolidation

Merge `Choose` into `Fork` with selection modes:

```rust
pub enum ForkSelect {
    All,                    // Execute all branches (current Fork behavior)
    One,                    // Execute first option deterministically (current Choose behavior)
    Prompt { prompt: String }, // LLM-selected option
}

pub struct Fork {
    pub threads: Vec<ForkThread>,
    pub select: ForkSelect,  // Default: All
    pub synthesize: Option<SynthesizeConfig>,
}
```

This simplifies the model: `FlowItem` becomes `Step | Fork`.

### 2. Flow Reference Variant

Add `FlowItem::FlowRef` for referencing another flow:

```rust
pub enum FlowItem {
    Step(Step),
    Fork(Fork),
    FlowRef(String),  // Reference to another flow by name
}
```

### 3. Flow Parents Data Model

A `ConcreteStep` carries its lineage:

```rust
/// A step with its provenance through nested flows.
pub struct ConcreteStep {
    pub step: String,              // The actual step name (e.g., "implement")
    pub flow_parents: Vec<String>, // Parent flows (e.g., ["grind", "ship"])
}

impl ConcreteStep {
    /// For commit messages: "grind ship implement"
    pub fn display_path(&self) -> String {
        let mut parts = self.flow_parents.clone();
        parts.push(self.step.clone());
        parts.join(" ")
    }
}
```

### 4. Flow Expansion

Flatten nested flows into concrete steps at execution time:

```rust
/// Expand a flow definition into concrete executable steps.
pub fn expand_flow(flow: &Flow, repo: &Path) -> Result<Vec<ConcreteItem>, FlowError> {
    expand_with_chain(flow, repo, vec![flow.name.clone()])
}

fn expand_with_chain(
    flow: &Flow,
    repo: &Path,
    chain: Vec<String>,
) -> Result<Vec<ConcreteItem>, FlowError> {
    let mut items = Vec::new();

    for item in &flow.items {
        match item {
            FlowItem::Step(step) => {
                items.push(ConcreteItem::Step(ConcreteStep {
                    step: step.name.clone(),
                    flow_parents: chain.clone(),
                }));
            }
            FlowItem::FlowRef(name) => {
                let nested = load_flow(name, repo)?;
                let mut nested_chain = chain.clone();
                nested_chain.push(name.clone());
                items.extend(expand_with_chain(&nested, repo, nested_chain)?);
            }
            FlowItem::Fork(fork) => {
                // Fork threads can also contain flow refs
                items.push(ConcreteItem::Fork(expand_fork(fork, repo, &chain)?));
            }
        }
    }

    Ok(items)
}
```

### 5. Flow Parents Storage

**Where flow_parents lives:**

| Component | Storage Location | Notes |
|-----------|------------------|-------|
| WaveRun (proto) | `repeated string flow_parents` | Add to WaveRun message, persisted per-tick |
| Wave (proto) | Computed from FlowInfo | Not stored directly; derived from current step_index |
| Rust store | `wave_runs` table | New column: `flow_parents TEXT` (JSON array) |
| Swift Wave | `sourceChain: [String]?` | Optional for backwards compatibility |
| Events | `WaveWaitingEvent.flow_parents` | Pass through in events |

**Proto changes:**

```protobuf
message WaveRun {
  string id = 1;
  string wave_id = 2;
  uint32 iteration = 3;
  uint32 step_index = 4;
  WaveRunStatus status = 5;
  string worktree = 6;
  string branch = 7;
  google.protobuf.Timestamp started_at = 8;
  optional google.protobuf.Timestamp ended_at = 9;
  optional string error = 10;
  repeated string flow_parents = 11;  // NEW: Path through nested flows
}

message WaveWaitingEvent {
  string wave_id = 1;
  string step = 2;
  string agent_id = 3;
  optional string wave_run_id = 4;
  uint32 step_index = 5;
  WaitingReason reason = 6;
  repeated string flow_parents = 7;  // NEW
}
```

**Rust store:**

```rust
// In wave_runs table, store as JSON array
pub struct WaveRunRecord {
    pub id: String,
    pub wave_id: String,
    pub iteration: u32,
    pub step_index: u32,
    pub status: WaveRunStatus,
    pub flow_parents: Vec<String>,  // Serialized as JSON
    // ...
}
```

### 6. Commit Message Integration

Update `autocommit` to accept flow parents:

```rust
// Current: lf implement: generated message
// New:     lf grind ship implement: generated message

pub fn autocommit(repo: &Path, step: &ConcreteStep, push: bool) -> Result<bool> {
    let prefix = format!("lf {}", step.display_path());
    // ... rest unchanged
}
```

Python equivalent update in `git.py`:

```python
def autocommit(
    repo_root: Path,
    task: str,
    flow_parents: list[str] | None = None,  # NEW
    push: bool = False,
) -> bool:
    # Build prefix: lf {chain...} {task}
    if flow_parents:
        prefix = f"lf {' '.join(flow_parents)} {task}"
    else:
        prefix = f"lf {task}"
    # ...
```

### 7. UI Considerations (Concerto)

**Option A: Breadcrumb Pills**
Show the full chain as expandable breadcrumbs:
```
[grind] > [ship] > [implement (2m)]  [compress]  [gate]  [consolidate]
```

**Option B: Nested Progress**
Hierarchical view when a step is actually a subflow:
```
[grind] > [ship ▼]
           ├─ [implement (2m)] ✓
           ├─ [compress] ←
           ├─ [gate]
           └─ [consolidate]
```

**Swift Model Update:**

```swift
public struct Wave {
    // Existing
    public var stepIndex: Int
    public var flowSteps: [String]?

    // New
    public var sourceChain: [String]?  // Current execution path
}
```

### 8. Fork Branches in Source Chain

Fork represents a branching point. Each branch gets its own flow parents extension:

```yaml
# grind.yaml
- fork:
    - step: security-review
    - step: performance-review
  select: all
```

When executing security-review inside grind, flow_parents = `["grind", "fork/security-review"]`.

For commit messages: `lf grind fork/security-review: ...`

### 9. Execution Flow

```
┌─────────────────────────────────────────────────────────────┐
│  Wave Config                                                 │
│  flow: "grind"                                              │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│  expand_flow("grind", repo)                                  │
│                                                              │
│  grind.yaml:        Expanded:                               │
│  - review           [ConcreteStep("review", ["grind"])]     │
│  - iterate          [ConcreteStep("iterate", ["grind"])]    │
│  - ship      →      [ConcreteStep("implement", ["grind", "ship"])]
│  - gate             [ConcreteStep("compress", ["grind", "ship"])]
│                     [ConcreteStep("gate", ["grind", "ship"])]
│                     [ConcreteStep("consolidate", ["grind", "ship"])]
│                     [ConcreteStep("gate", ["grind"])]       │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│  WaveExecutor                                                │
│  tick_index = 3  →  ConcreteStep("compress", ["grind","ship"])
│                                                              │
│  commit: "lf grind ship compress: optimize image assets"    │
└─────────────────────────────────────────────────────────────┘
```

### 10. Migration Path

1. **Proto**: Add `flow_parents` fields (backwards compatible - new field)
2. **Rust store**: Migration to add `flow_parents` column with default `[]`
3. **Python**: Update `autocommit()` signature, maintain backwards compat
4. **Swift**: Add optional `sourceChain`, update FlowProgressPills
5. **Executor**: Update to use expanded flow representation

## Open Questions

1. **Fork branch naming**: Use `fork/branch-name` or `fork[0]` for anonymous threads?
2. **Cycle detection**: How deep to allow nesting? Suggest max depth of 5.
3. **Flow definition validation**: Validate at parse time or expansion time?
4. **UI for deep nesting**: Collapse after N levels?

## Non-Goals

- Dynamic flow modification at runtime
- Conditional flow branching based on step results (beyond Choose/Fork)
- Cross-repo flow references
