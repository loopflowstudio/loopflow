# Settle a Flow after its PR merges

Jack Heart authorized autonomous Infrastructure delivery. LOO-390's PR #1474
merged at 2026-10-07T00:29:38Z, but its managed Flow remains at the final
`pr land` operation. Steps 0–7 are recorded complete. Its checkout exists,
there is no pending review or live driver, and the Task has no active PR.

`lf --task LOO-390 flow start` fails in `restore_task_checkout` before looking
at the completed landing. `continue_task_async` independently refuses the
same between-PR state. `flow resume` routes through the same Task path.
`flow end` would retire the Flow by request, not consume the successful landing.

Repair the distinction between consuming an existing operation result and
starting new work. Use the exact operation-bound landing and existing Flow
cursor/checkpoint authority. Do not infer completion from any merged PR, create
an empty successor, rewrite history, or mark the Task complete. Preserve the
remaining storage outcome and installed acceptance.

Next: inspect the cursor transition API and implement settlement under the
existing Flow driver lock/version/claim rules. Verify a merged final landing
finishes without an active PR; a pending or wrong landing does not advance;
live or unresolved execution remains protected; and a nonfinal landing does
not execute subsequent work without its normal prerequisites. Use disposable
stores for tests. Install the released repair before retrying the real Home.

Check: `cargo test -p loopflow --lib merged_landing_settles_without` PASS; pending landing stays, final landing ends without successor, final-checkpoint interruption resumes, and next review remains unlaunched. Final all-target Clippy pending.
