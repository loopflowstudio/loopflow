
### Installed capture counterexample and Task advance

The installed live-data capture is ux-reference-images/installed-live-workspace.png.
It displays three Waves and a populated planning read, but several task titles,
Work heading, search placeholder, and toolbar labels render nearly white on the
light background. This is an observed regression, not a successful visual demo.
Hypothesis: default system foreground inherits dark appearance while custom
backgrounds use the light palette. Resolve foreground/appearance ownership and
verify both appearances through the installed path; do not infer cause from the
screenshot alone.

The first `lf task resume LOO-291` reported no active Flow after the local upgrade.
`lf task run LOO-291 --flow slice --json` then succeeded. The command chose the
existing Task/worktree; it did not create a duplicate Task. The shared status/runs
surfaces do not yet expose a new active worker clearly, so command acceptance is
recorded without claiming observed provider progress.

### Final handoff state

The human reported closing the app; leave it closed. Closing is not launch-demo
confirmation. Configured conversation launch remains unverified.

The new Task Flow is slice / implement with worker Run
run_643d86d70d5b4109a460e80a87a97533. Inspection through the machine-selected
installation (without this chat's stale published LF_HOME/LF_CONTROL_HOME
observability overrides) reads the new Run and its growing provider output.
The visual counterexample is durable Task steer event 65123, despite the command
returning an owner-evidence error after recording the message. Do not duplicate
the Task or kill any unclaimed provider process. No full Task completion,
configured demo confirmation, or PR landing is claimed.
