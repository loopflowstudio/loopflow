# Built-in Flows

Flows shipped with loopflow. Available everywhere without user configuration.

| Flow | Steps | Use case |
|------|-------|----------|
| ship | implement → reduce → polish | Full feature workflow |
| quick | implement → polish | Fast iteration |
| iterate | review → implement → polish | Improve existing code |
| reduce | reduce → polish | Simplify bloated code |
| roadmap | Fork(roadmap×3) → Synthesize | Strategic planning with multiple perspectives |

## Fork Execution

Fork runs multiple agents in parallel, each in its own temporary worktree. Synthesis combines results into the parent worktree.

### Process Diagram

```
Agent (persistent worktree @ branch-1)
│
├── Fork creates temp worktrees from HEAD
│   ├── fork-flow-1/ (goal: infra-engineer)  ─┐
│   ├── fork-flow-2/ (goal: designer)        ─┼─ run in parallel
│   └── fork-flow-3/ (goal: product-engineer)─┘
│
├── Each fork runs its step independently
│   └── Commits stay in fork worktree
│
├── Synthesize reads all fork diffs
│   └── Writes unified result to parent worktree
│
├── Cleanup: delete fork worktrees
│
├── Flow continues (reduce, polish, etc.)
│
├── PR created → main (with --auto merge)
│
└── move_worktree() → fresh branch for next iteration
```

### Worktree Lifecycle

**Persistent (agent's worktree):**
- Created once when agent starts
- Survives across iterations
- `move_worktree()` switches branches, keeps directory

**Ephemeral (fork worktrees):**
- Created at Fork start
- Named: `{repo}.fork-{flow}-{n}`
- Deleted after Synthesize completes

### Fork Definition

```python
Fork(
    {"goal": "infra-engineer"},
    {"goal": "designer"},
    {"goal": "product-engineer"},
    step="roadmap",           # applied to all
    synthesize={"goal": "ceo"},  # synthesis config
)
```

| Field | Description |
|-------|-------------|
| `*agents` | Dicts with goal, model, step, area |
| `step=` | Default step for all agents |
| `model=` | Default model for all agents |
| `synthesize=` | Config dict with goal, area, prompt |

### Synthesis

Synthesize receives context about all fork outputs:
- Git diff from each fork (changes vs base commit)
- Fork config (goal, model, etc.)

The synthesizer writes:
1. Analysis to `scratch/synthesis.md`
2. Unified implementation to the parent worktree

## Adding a Built-in Flow

1. Create `{name}.py` with explicit import: `from loopflow.lf.flows import Flow, Fork`
2. Return a `Flow` with steps
3. Update this README
