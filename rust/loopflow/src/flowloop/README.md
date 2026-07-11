# Wave runtime

`wave.rs` drives the permanent Wave mind. Inbox messages, heartbeats, and
crons schedule provider-backed flow steps while the listener owns the journal,
thread, and crash recovery.

Concrete file-writing work leaves the Wave home through a Linear-backed Task
Session:

```bash
lf task run INF-123
lf task steer INF-123 "also rename the flag"
lf task wait INF-123
```

Each Task Session owns one immutable sibling worktree, provider transcript,
and pull request to `main`. Projects stay directives to the Wave; they do not
own processes or worktrees.
