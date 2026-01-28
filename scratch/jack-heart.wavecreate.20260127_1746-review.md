# Review: Wave Creation with Worktree

## What was implemented

`lfd create` now reports the worktree it creates and auto-switches the user there via shell integration.

Before:
```
Created swift-falcon (abc123)
  Repo: /path/to/repo

Configure before running:
  lfd area swift-falcon src/
  ...
```

After (worktree success):
```
Created swift-falcon (abc123)
  Worktree: ../loopflow.swift-falcon.main
  Branch: swift-falcon.main

cd ../loopflow.swift-falcon.main
```

After (worktree failure):
```
Created swift-falcon (abc123)
  Repo: /path/to/repo
  (worktree creation failed)
```

## Key choices

1. **Removed verbose "configure before running" instructions** — The old output showed 4 lines of example commands. Since waves now auto-create worktrees and are designed for immediate interactive use, this instruction block was noise. Users who need configuration can run `lfd show <name>` or check `lfd --help`.

2. **Shell directive pattern matches existing code** — Used the same `write_directive` + fallback echo pattern as `lfops wt create` and `lfops wt switch`. If shell integration isn't installed, the user sees a copy-pasteable `cd` command.

3. **Warning for worktree failure stays minimal** — Just `(worktree creation failed)` in yellow. The wave still exists and can be used, so we don't want to alarm the user.

## How it fits together

```
lfd create
    ↓
create_wave(repo, name)
    → creates worktree via create_worktree()
    → returns Wave with worktree/branch fields set (or None if failed)
    ↓
CLI outputs worktree info if available
    ↓
write_directive("cd ...") triggers shell to switch directories
```

## Risks and bottlenecks

- **Shell integration dependency**: The auto-switch only works if user ran `lfops shell install`. Fallback behavior (echo the cd command) is fine but requires manual copy-paste.

- **Worktree creation can silently fail**: If `create_worktree` fails (branch collision, git issues), the wave still gets created but without a worktree. User sees a warning but might not understand why. This is existing behavior, not introduced by this change.

## What's not included

- No changes to the HTTP API or Concerto — the `worktree` field was already returned by `POST /waves`.
- No new tests — existing tests pass, and the change is a display-only modification. The underlying `create_wave` function and `write_directive` are already tested.
