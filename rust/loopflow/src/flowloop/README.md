# Wave runtime

`wave.rs` drives the permanent Wave mind. Inbox messages, heartbeats, and
crons schedule provider-backed flow steps while the listener owns the journal,
thread, and crash recovery.

Concrete file-writing work leaves the canonical main control plane through a
Linear-backed Task Session:

```bash
lf task run INF-123 --name parser-recovery --directive "fix the parser before the docs"
lf task steer INF-123 "also rename the flag"
lf task receipt COMMAND_ID --until incorporated --timeout 30s
lf task wait INF-123
lf task resume INF-123 --model codex --reason "Claude quota exhausted"
```

Each Task Session owns one stable sibling worktree. Provider processes and
transcript handles are replaceable body generations; a model handoff preserves
the Session, directive, worktree, and PR chain.
Ordered PRs own its serial branches; a merge settles one PR, while only
`lf pr land -c` or `lf task complete` requests Task completion. The resident
Task runs kickoff once, repeats its selected inner flow, and settles only after
its gate approves; gate repairs return it to another iteration.
A Project Session owns the bounded KR-pursuit process that creates and
supervises Tasks, but no worktree, branch, or PR.
