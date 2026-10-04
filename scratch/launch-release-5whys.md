# Worker startup and release settlement: causal analysis

2026-10-03. Scope comes from `launch-release-incident-report.md`, which arrived
during inspection. Preserve that report as the original observations. This pass
read source and Git history; it did not launch workers, change runtime data,
publish a release, or independently refresh remote/runtime status.

## Impact and recovery

Jack's reported LOO-371/372 workers timed out before producing worker Sessions.
The report subsequently observed LOO-372 running its saved implement invocation;
LOO-371 remained idle. LOO-366 resumed its existing invocation through an isolated
tmux server. That is workaround evidence, not proof that both reported timeouts
share one cause or that the launch implementation is repaired.

The report records v0.13.0's PR #1420 merged and candidate passing, but no published
GitHub Release at its latest read. One release attempt failed on `invalid stored
landing placement: home`; the retry lacks a terminal receipt and has unresolved
execution authority. Release recovery remains unverified. Absence from a process
listing alone does not authorize overlapping publication.

## Startup chain: supported hypothesis with an evidence limit

1. The parent reports a ten-second timeout when no accepted worker handoff,
   terminal Task, Flow completion, review boundary, or recorded failure appears.
   `ops/task.rs::wait_until_running` observes those outcomes; timeout itself does
   not identify the child failure. `ops/child.rs::CHILD_STARTUP_GRACE` sets ten seconds.
2. A detached tmux launch can succeed while its shell or lf child exits before
   admission. `engine/process.rs::start_tmux_session` checks the tmux client status;
   the worker's startup stderr is not captured there. The parent then waits for
   a claim transfer that never arrives.
3. A prior real A/B probe reproduced one such pre-admission failure: the existing
   tmux 3.7c server held a deleted checkout as cwd, and even `new-session -c` with
   a valid checkout produced an unresolvable child cwd. Explicit child `cd`
   restored pwd; the same installed lf and Home passed Task status on an isolated
   server. Both login and non-login shells failed on the old server, so login
   shell initialization was not necessary to reproduce it.
4. Current source still delegates cwd establishment entirely to tmux `-c`.
   `lf_session_shell_command` clears inherited authority and execs lf without
   establishing cwd inside the child. Thus the reproduced environment defect
   remains possible despite context scrubbing and detached-client changes.

The probe is recorded in
`/Users/jack/src/loopflow.make-the-release-and-ci/scratch/loo-326-worker-startup.md`.
It concerns LOO-326, not the exact LOO-371/372 children. Original child stderr was
unavailable after panes exited with remain-on-exit off. Why tmux ignored the
intended cwd in that environment remains unexplained. LOO-372's later progress
also contradicts any claim that this is an unconditional launch failure.

**Proposed prevention:** establish the shell-quoted requested cwd in the child
before exec; retain bounded, secret-safe startup failure output tied to the
existing launch identity. Prove this with a headless isolated tmux server whose
original cwd is removed, then launch into an existing checkout and observe child
cwd and admission. Exercise a failing cwd too: the parent should expose the
actual failure. Finally verify supported continuation preserves each Task and
saved Flow. Increasing the timeout cannot repair an already exited child.

## Landing chain: confirmed retained-data incompatibility

1. `store/sqlite/pr_landings.rs::map_landing` accepts null or `local` placement;
   `home` returns the exact error in the report.
2. `0.12.14.001_release.sql` explicitly allowed `home`, required its Home ID, and
   retained supervisor PID and heartbeat. This was valid released data.
3. Commit `d296b4805` (#1296) removed the `home` reader arm and its Home identity
   from the runtime representation. The current migration sources contain no
   corresponding conversion of that landing placement. Removing an execution
   model therefore narrowed the reader without retiring its persisted states.
4. `pending_pr_landings` collects decoded rows into one Result. One incompatible
   pending row fails the entire repository read, before
   `ops/pr_landing.rs::reconcile_repository_async` can reconcile any landing.
   This explains the breadth of the recorded reconciliation failure. The exact
   v0.13.0 release call stack was not supplied, so the report establishes its
   release impact, not an independently reproduced publisher trace.
5. The inspected migration regression,
   `landing_repair_counter_removal_preserves_supervision`, seeds only `local`
   supervision and compares SQL values. It cannot detect a retained `home` row
   that migrates successfully but fails the domain reader. This is a concrete
   coverage gap, not evidence that all migration testing was absent.

**Proposed prevention:** repair the released-data transition with explicit
ownership semantics, preserving landing identity, delivery intent, and evidence
needed to fence an old supervisor. Do not merely relabel `home` as `local` or
delete the offending row: the former can misinterpret a historical PID and the
latter discards the obligation. Test a released-frontier database containing
both placements through migration and the actual landing reader/reconciler;
prove retained intent, repeatability, and rejection of stale supervisor writes.
Then recover the same release through supported re-entry once exact authority
is established. No repair or migration was made by this analysis.

## Planning boundary

The older branch repairs for LOO-367 historical Exec admission and LOO-295 missing
PR identity have recorded installed acceptance in infrastructure memory. They do
not repair either failure above. Source fixes, installed acceptance, and release
publication are separate claims.

Next action: use these two bounded prevention proposals in launch-plan, preserve
LOO-372's running invocation, refresh LOO-371 before continuing it, and resolve
the existing release retry's authority before re-entry. The proposals are analysis
output; this note does not expand the incident report's authorization.

Check: source/history inspection confirms landing reader/schema mismatch and the
unrepaired tmux cwd dependency; no code changed, so prior tests were not rerun.
