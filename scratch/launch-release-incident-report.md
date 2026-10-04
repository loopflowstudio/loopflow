# Launch and release incident — Jack Heart, October 3, 2026

Jack requested `lf incident -b` for LOO-371/LOO-372 worker startup timeouts, then autonomous delivery and landing without interactive Flow steps. Jack extended the same incident treatment to stalled v0.13.0 publication and `invalid stored landing placement: home`. Publication and landing of these repairs are authorized. Preserve other active workers and all retained history.

Observed: both Tasks had saved `code` Flows at implement without worker Sessions. Latest read now shows LOO-372 running implement in invocation 82c166ae-3233-4005-8437-e6926649be1b; do not replace it. LOO-371 was idle in a3ff7e90-0ffb-4b05-9089-5945439cfc1e. Recheck before continuation. An isolated TMUX_TMPDIR with TMUX unset successfully recovered LOO-366 startup in this operator pass; its same saved feature invocation reached kickoff. Historical deleted-server-cwd diagnosis is in /Users/jack/src/loopflow.make-the-release-and-ci/scratch/loo-326-worker-startup.md.

Release v0.13.0 PR https://github.com/loopflowstudio/loopflow/pull/1420 merged; candidate https://github.com/loopflowstudio/loopflow/actions/runs/37182312803 passed. GitHub Release v0.13.0 was absent at latest read. Minor attempt a92a95e8-e841-41c5-b898-1ead9f48ea9e failed on stored landing placement `home`. Retry ff94aac9-b59d-409b-9bce-45dc921f1f1a has no terminal receipt; `lf mon ps` did not show that identity. Unknown liveness is not permission to overlap publication. Caller session_da543c38db3a41adaa8a5c786d5827e5 has a recorded failed outcome. Establish exact current release authority before recovery.

The incident Flow launched without a positional report; this file supplies the concrete report for unbreak, 5whys and launch-plan. Repair through supported lf operations, prove original workflows, retain failure evidence, and deliver prevention autonomously. No raw database rewrite or destructive reset is authorized.

## Recovery in progress — October 4

Jack Heart explicitly assigned both defects to Infrastructure and authorized
implementation, verification, publication, landing and release recovery in this
checkout. This supersedes the design's allocation deferral. No replacement
worker or interactive review Flow was launched; LOO-371/372/326 were not changed.

The retained receipt for retry ff94aac9-b59d-409b-9bce-45dc921f1f1a identifies
PID 17052, now absent. Nonblocking OS-lock checks found the exact minor lease
and v0.13.0 preparation lease unheld. Its terminal outcome remains unrecorded;
the receipt and lease evidence establish that it no longer owns publication.
GitHub refreshed the same passing candidate and absent Release before re-entry.
`lf release run minor` now owns Exec a7dee370-c624-4ac5-b063-a981714d7e7d and
resumed the saved v0.13.0/v0.12.32 pair, including its original prepared tree.
Signing completed and the publisher is running; a draft Release is not completion.

The repair retains obsolete authority on each landing, fences its generation,
rejects fresh Home claims by old executables, and preserves local supervision.
Detached child commands explicitly enter their shell-quoted cwd; the tmux client
also starts there, reporting a missing cwd at spawn. The original timed-out
children's stderr is unavailable after their panes exited; no reconstructed log
is presented as original evidence. Existing stderr inheritance remains intact.

Check: 25 focused landing tests, expanded migration/read/reconciliation/repeat regression,
10 process tests (including isolated tmux 3.7c with deleted server cwd),
`cargo clippy --all-targets -- -D warnings`, and migration catalog check passed;
final CLI build and publication remain pending. Infrastructure owns LOO-373.
