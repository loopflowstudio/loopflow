---
requires: A merged PR and the Task's Done when claims
produces: A record of quick checks on main and any follow-up
---
Check the merged change on main and finish the Task or fix forward.

Read the outcome and the merge SHA. Inspect the push CI result once and run
a fast, bounded check on the merged code that exercises the promised behavior.
Use an already available release or deployment when it matches the merge.

Record the commands, results and anything still pending in `scratch/`.
If a promised behavior fails, file a linked fix-forward Task with the failing
command, its output and the expected result.
If a check needs a later release, a long build or access elsewhere, record
what remains and where it can be checked, filing a linked follow-up when
work remains. Advance as soon as the available checks finish.
