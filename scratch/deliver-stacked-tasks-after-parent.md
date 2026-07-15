# Task-owned stacked delivery

## Contract

`Task` remains the durable owner of one readable, flat worktree and a serial
sequence of PRs. Cross-Task dependency is placement, not PR rotation:

```bash
lf task run CHILD --stack-on PARENT
```

The child gets its own worktree and branch, forked from the parent Task's active
published PR. Its first PR targets the parent branch automatically. Branch names
remain readable hints; the durable parent PR id and fork commit carry the stack.

After the parent merges, `lf pr land` replays only child-authored commits onto
`main`, retargets the child PR, and clears the parent link. This must survive a
squash merge without patch-id guessing.

## Remove from the current draft

- `lf pr stack`
- multiple simultaneously open PRs owned by one Task
- branch rotation as the way to create dependent work

Same-Task multi-PR delivery remains serial through `lf pr land --next` and
`lf pr next`. Separate dependent work gets a separate Task and worktree.

## Proof

- CLI parsing accepts `--stack-on` for both existing and newly created Tasks.
- initial Task placement forks from the parent PR branch and records its exact tip.
- PR creation targets the recorded parent branch without exposing a base flag.
- parent branch movement rebases only child work and advances the durable fork.
- parent squash merge collapses only child work onto `main` and retargets its PR.
- ordinary Tasks and serial follow-up PRs retain their current behavior.
