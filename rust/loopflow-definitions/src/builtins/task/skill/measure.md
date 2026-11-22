---
requires: a number to establish, or a change to compare against one
produces: a vetted measurement with its verdict and limiter
action_style: procedural
---
Get a number you can trust, and say what limits it.

Write the claim you expect to make in the words you would ship, such as "export
is 30% faster at p50 on the 60k-row dataset". The checks below test that sentence.

Use the metric, direction and workload the request names. Find the repository's
existing harness first; build the smallest one that prints the metric, an error
count and a count of the work done when none exists. Pick a workload that shows
the problem and confirm the harness can tell it from an easier one. Once a
comparison starts the harness is frozen. Changing it starts a new baseline.
Never create artificial load to get a number.

Answer each question from a run, not from reading code:

1. **What limits it?** Profile in a run you do not report and map the hot spot
   to source. A difference you cannot explain is inconclusive.
2. **Is each side tuned?** Run both sides the way production runs them: same
   build, flags, data, versions and cache state.
3. **Does it fit the limits?** Do the arithmetic. Removing a piece that takes
   10% of a run makes it at most about 11% faster.
4. **Did it error?** Count failures, and check outputs are correct, not just
   present. Rejections are fast and retries are slow.
5. **Does it reproduce?** Run each side at least 5 times, alternating. Report the
   median and range. A gap inside the run-to-run range is no measurable difference.
6. **Does it matter?** Report a micro result as its share of the path someone waits on.
7. **Did the work happen?** Confirm the work ran inside the timed region.
8. **Did behavior hold?** Compare outputs or a digest. Changed behavior
   invalidates the speed comparison.

Lead with the verdict: better, worse, no measurable difference or inconclusive.
Give the number with its unit, run count, range and limiter. Missing runs are
missing, never zero. Inconclusive is a valid result; say what would settle it.

## Comparing a change

Compare the current change with the last kept commit, building both. Keep it
when it clears the noise and behavior holds. Otherwise return the tree to the
last kept commit, including anything the change motivated. Simplicity counts: an
equal number with less code is a keep, and a tiny gain that adds real complexity
is not. Add one row to the table in `scratch/measure.html` either way: commit,
value, verdict, hypothesis, limiter. Keep it one self-contained page, with the
latest verdict above the table.
