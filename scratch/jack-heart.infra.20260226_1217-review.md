# Multi-Step Fork Branches — Design Review

## What was implemented

Fork branches can now run multiple steps sequentially. Previously, each fork branch ran exactly one step. Now a branch can reference a flow (via `flow: build`), and the entire flow's steps execute sequentially within that branch's worktree. Both CLI (`lf flow`) and daemon (`lfd`) executors support this.

**Concrete changes:**

- `ConcreteForkBranch` struct wraps `Vec<ConcreteStep>` + directions + label, replacing the single `ConcreteStep` per branch
- Fork YAML parsing accepts three formats: `step:` shorthand, `flow:` shorthand, and explicit `branches:` with per-branch `step:` or `flow:` keys
- `expand_fork` resolves flow references into multi-step branches, rejecting nested forks within branches
- `ForkManifestBranch.steps: Vec<ForkManifestStep>` records per-step exit codes (replacing the single `step: String` field)
- Both executors loop through steps within each branch with fail-fast semantics
- Duplicated flow-expansion heuristic extracted into `is_multi_step_flow()`

## Key choices

| Decision | Why | Alternative rejected |
|----------|-----|---------------------|
| Flow refs as composition mechanism | No new `steps:` syntax. Flows already compose steps. | Inline `steps:` list on fork YAML — redundant concept |
| Fail-fast within branches | First non-zero exit stops the branch. Remaining steps skipped. | Continue-on-error — complicates manifest interpretation |
| Nested forks rejected | Sequential steps only within branches. Keeps executor and manifest flat. | Recursive fork-in-fork — exponential complexity for no proven use case |
| `ConcreteForkBranch` as new type | Clean separation of branch-level metadata (directions, label) from individual steps | Tuple or `Vec<ConcreteStep>` directly — loses type clarity |

## How it fits together

```
YAML parse → FlowItem::Fork { branches: Vec<FlowItem> }  (unchanged parse representation)
         ↓
expand_fork → ConcreteFork { branches: Vec<ConcreteForkBranch> }  (new: multi-step branches)
         ↓
plan_fork_execution → Vec<ForkBranchExecutionPlan>  (steps + directions per branch)
         ↓
executor: for each branch (parallel) { for each step (sequential) { launch_agent } }
         ↓
ForkManifest { branches: [{ steps: [{ name, exit_code }], ... }] }
```

The parse layer is largely unchanged — `FlowItem::Step` still represents both steps and flow references at parse time. The expansion layer resolves flow references into concrete step sequences. The executor loops sequentially within each parallel branch.

## Risks and bottlenecks

- **Manifest schema change is breaking.** Any existing `fork-manifest.json` files with the old `step: String` field won't deserialize. This is fine — manifests are ephemeral (written per-run, consumed by synthesize, then deleted). No migration needed.
- **Long-running branches.** A 5-step branch blocks its worktree for the full duration. The scheduler slot is held for all steps. This is acceptable — branches are supposed to be coherent units of work.
- **Error attribution.** If step 3 of 5 fails, the manifest records which step failed and its exit code. The synthesize step can inspect this. But the user-facing error message in `lf flow` just says "fork branch N failed" without naming the step. The daemon executor logs step-level detail via tracing.

## What's not included

- **Conditional steps / `when` predicates**: Deferred to a separate milestone.
- **Nested forks within branches**: Explicitly rejected. If a referenced flow contains a fork, expansion fails with a clear error.
- **Per-step direction overrides within a branch**: All steps in a branch run with the branch's directions. No per-step direction switching.
- **Interactive steps in fork branches**: Still rejected — same as before.

## Validation

```
cargo fmt --all -- --check         ✓
cargo clippy -p loopflow -- -D warnings  ✓
cargo test -p loopflow flow        ✓ (38 tests, 0 failures)
cargo test -p loopflow fork        ✓ (23 tests, 0 failures)
tests/e2e/test_smoke.sh           ✓
```

All existing tests pass unchanged. 4 new unit tests in `flow.rs` (parse + expansion), 3 new unit tests in `fork.rs` (planning + manifest serde), and 1 updated integration test in `flow_tests.rs`.
