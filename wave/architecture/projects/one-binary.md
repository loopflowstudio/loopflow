# One binary

Collapse the three binaries — `lf`, `lfd`, `lfq` — into one story. Audited
2026-07-08: most lfd mutation handlers already delegate to the same
`crate::ops::*` functions `lf` calls; the exec door re-execs `lf` argv; the
queue logic is already consumed by `lf op queue reconcile`. `lfq` is NOT
removed — it is the live exec-door client (bin/lfq.rs) sandboxed subagents
use, and needs its own collapse story.

## KRs

- The named collapse-blockers each get a home or die: the resident HTTP
  listener socket; GitHub webhook ingress + signature verification + CI
  dedupe cache (hooks.rs, TODO(M1/M3)); the machine-global token-refresh
  loop; OS service management (launchd/systemd); boot hygiene
  (fail_orphaned_runs, reconcile_sessions, worktree janitor); the bearer-
  auth boundary; control-session tmux lifecycle.
- `lfq` folds into `lf` (e.g. `lf exec` hitting the door) or is explicitly
  re-chartered; coordinate with systems' zero-hitl exec-door KR.
- No second execution path survives: git, tmux, vendor, and worktree paths
  all route through `lf`.
