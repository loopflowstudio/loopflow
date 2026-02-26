# Multi-Step Fork Branches (Infra Pass 3, Milestone B)

## Problem

Fork branches run one step and hand off to synthesize. `implement → compress → lint` per direction would produce higher-quality drafts before synthesis. Today each branch gets one step — shallow output, heavy synthesis burden.

Wave goals advanced: "Invest in the flow system (the differentiators)" and "Faster reactions and richer wave composition."

## Approach

Allow fork branches to reference named flows. A flow ref that expands to multiple steps runs them sequentially in the branch's worktree. No new `steps:` syntax — if you want a multi-step branch, you make a flow and reference it.

### Type changes

```rust
// Before
pub struct ConcreteFork {
    pub branches: Vec<ConcreteStep>,
    pub flow_parents: Vec<String>,
}

// After
pub struct ConcreteFork {
    pub branches: Vec<ConcreteForkBranch>,
    pub flow_parents: Vec<String>,
}

pub struct ConcreteForkBranch {
    pub steps: Vec<ConcreteStep>,  // 1+ steps per branch
    pub flow_parents: Vec<String>,
    pub label: String,
}
```

### YAML syntax

```yaml
# Single-step (unchanged)
- fork:
    step: reduce
    drafts:
      - direction: infra
      - direction: ux

# Multi-step via flow ref — each draft runs the full "build" flow
- fork:
    flow: build
    drafts:
      - direction: infra
      - direction: ux
      - direction: ceo

# Explicit branches with different flows
- fork:
    branches:
      - flow: build
        direction: infra
      - step: review
        direction: ux
```

### Expansion changes in `expand_fork`

Remove the single-step enforcement on flow refs. Today:

```rust
[ConcreteItem::Step(step)] => (step.step.clone(), name.clone()),
_ => return Err("fork flow ref {name} must expand to a single step")
```

After: a flow ref expands to `Vec<ConcreteStep>`. Nested forks within branches remain rejected — if a referenced flow contains a fork node, that's an error.

### Execution changes in fork executor

Each branch's async task loops through its steps sequentially:

```
for step in branch.steps:
    build_step_prompt → launch_agent
    if exit_code != 0: break (fail-fast)
```

All branches still run in parallel (each in its own worktree). Synthesize still runs after all branches complete.

### Fork manifest extension

```rust
pub struct ForkManifestBranch {
    pub index: usize,
    pub steps: Vec<ForkManifestStep>,  // replaces single `step` field
    pub direction: String,
    pub worktree: String,
    pub branch: String,
    pub exit_code: i32,               // first non-zero or 0
}

pub struct ForkManifestStep {
    pub name: String,
    pub exit_code: i32,
}
```

### Fork planning

`plan_fork_execution` accepts `&[ConcreteForkBranch]` instead of `&[ConcreteStep]`. Each plan entry carries `steps: Vec<ConcreteStep>` instead of a single step. Interactive step validation applies to all steps in a branch.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| `steps:` (plural) syntax on fork YAML | Inline multi-step without defining a flow | Redundant concept. Flows already compose steps. "If you want a list of steps, make a flow." |
| Fork branches as full sub-flows (recursive `Flow` type) | Maximum flexibility — branches could contain nested forks | Over-engineered. Sequential steps cover the real use case. Nested forks add recursive complexity to executor and manifest. |
| `when` predicates on steps for conditional execution | Per-step skip logic based on activation context | Deferred. Stimulus→flow routing (shipped separately) covers the primary use case. `when` predicates can be added later if needed. |

## Key decisions

1. **Flow refs are the composition mechanism.** No new `steps:` syntax. Single-step branches use `step:`, multi-step branches use `flow:`. The flow system already handles composition.

2. **Fail-fast within branches.** If any step in a branch exits non-zero, the branch stops. Remaining steps are skipped. The branch reports the first non-zero exit code.

3. **Nested forks remain rejected.** A flow ref that expands to contain a fork node is an error. Sequential steps only within branches.

4. **Fork manifest records per-step outcomes.** The synthesize step sees which steps each branch ran and their individual results.

## Scope

**In scope:**
- `ConcreteForkBranch` struct with `steps: Vec<ConcreteStep>`
- Remove single-step enforcement in `expand_fork`
- `flow:` key in fork YAML parsing (alongside existing `step:`)
- `plan_fork_execution` updated for multi-step branches
- Sequential step execution within fork branches in executor
- `ForkManifestBranch` updated with `steps: Vec<ForkManifestStep>`
- Unit tests for multi-step fork expansion
- Integration test for fork with multi-step branches

**Out of scope:**
- `when` predicates / conditional steps (deferred)
- Activation payload persistence (deferred)
- Decision persistence / replay (deferred)
- Nested forks within branches

## Implementation plan

| Order | What | Files | Tests |
|-------|------|-------|-------|
| 1 | `ConcreteForkBranch` struct, update `ConcreteFork` | `engine/flow.rs` | Unit: struct construction |
| 2 | Remove single-step enforcement in `expand_fork`, accept multi-item flow refs | `engine/flow.rs` | Unit: flow ref expanding to multi-step |
| 3 | `flow:` key in `parse_fork_value` | `engine/flow.rs` | Unit: YAML parsing round-trip |
| 4 | Update `plan_fork_execution` for `ConcreteForkBranch` | `engine/fork.rs` | Unit: planning with multi-step branches |
| 5 | `ForkManifestStep`, update `ForkManifestBranch` | `engine/fork.rs` | Unit: manifest serde |
| 6 | Sequential step loop in fork executor | `executor/wave/fork.rs` | Integration: fork with multi-step branches |

## Validation

```bash
cargo fmt --all -- --check
cargo clippy -p loopflow --all-targets -- -D warnings
cargo test -p loopflow flow
cargo test -p loopflow fork
tests/e2e/test_smoke.sh
```

## Done when

- Fork branches with `flow: build` run the full flow sequentially in one worktree — verified by test.
- Fork branches with `step: reduce` still work unchanged (backwards compatible).
- Fail-fast: a non-zero exit in step 2 of 3 stops the branch — verified by test.
- Fork manifest records per-step outcomes — verified by manifest JSON inspection.
- All existing flow/fork tests pass unchanged.
