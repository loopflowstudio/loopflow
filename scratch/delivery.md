# LOO-487 delivery

Jack Heart approved the demo (“lg2m, ship it”) and requested shipping LOO-487,
then publishing a minor release containing it. This supersedes the demo hold.
The accepted design and gate evidence remain at
`aabba6342d:scratch/terminal-polish.md`; landing cleared added scratch normally.

Gate completed with a stale-PR-state repair. Static checks passed; affected tests,
app builds and renderer validation are deferred to capable CI because local free
space is below the repository floor. Prior passes do not validate the new repair.

Landing refused because parent PR #1531 remains open. The Task runner's automatic
second gate was interrupted. Preserve the completed gate; do not replay it to
bypass the dependency. Parent landing coordination is pending in this conversation
while LOO-462 retains its active conversations.

After the parent merges, finish sync and landing, verify LOO-487 completion, then
run `lf release run minor`. Verify that the released snapshot includes LOO-487.
The release owner completes any required closing patch and the minor from the
same product state; do not start release before this Task is included.
