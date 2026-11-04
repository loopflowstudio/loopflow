# Accepted cleanup rules — 2026-11-04

Jack Heart accepted all four proposed rules in the Task conversation:

- Occasional 10–30 minute rebuilds are acceptable for inactive worktrees.
  Preserve useful caches for actively used work and preserve live output.
- Unused reproducible dependencies are disposable: virtual environments,
  node_modules, downloaded packages, Docker images and build caches.
  Docker volumes require separate inspection; this does not approve deleting
  original data or unpublished local images that cannot be recreated.
- Disposable successful-run logs, screenshots and temporary homes retain 24
  hours; failed-run evidence retains seven days. Explicitly saved evidence stays.
  Determine run outcome from evidence, not an assumed successful exit. Durable
  conversation history is not disposable test evidence. Reconcile existing
  generic pytest/demo retention with this accepted distinction.
- Enforce bounded caches and remove old build generations even with plentiful
  free space. Select and document concrete machine-local size limits from the
  observed workload; the conversation did not prescribe numeric caps.

Keep these rules in the existing hourly launchd runner. Continue measuring growth
and preventing recurrence across coding repositories. Existing source,
unpublished-work, history/database and explicitly saved evidence protections
remain. The two project-owned experiment stores remain excluded. The accepted
4h/1h/15m idle-build windows and 24h minimum before stopping eligible owners are
unchanged; size policies must preserve live work and explicit reservations.

The active implementation Graph owns integration, tests, local installation and
scheduled acceptance. These are accepted requirements, not a claim of deployment.
Keep personal paths and inventory in private machine configuration, and keep this
utility out of product packages, DMGs and automatic installation for other users.
