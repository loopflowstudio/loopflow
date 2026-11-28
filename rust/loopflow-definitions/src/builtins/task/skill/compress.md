---
requires: the branch's changes against its base, and the plan if one exists
produces: a simpler change and a simpler area around it, in commits that say what each cut removed
action_style: procedural
---
Simplify everything this branch touches until it is easy to understand and change, keeping the behavior people rely on. The output is a diff.

Treat the branch as one change against its base. Follow it through its
callers, tests, configuration and docs. Take every worthwhile simplification
there. Don't stop after one small win, and don't wander into unrelated cleanup.

1. **Read the whole change against its base** and the plan if one exists.
   Passes built by separate contexts leave seams that only the whole shows.

2. **Cut deepest first.** Remove obsolete concepts and paths before polishing
   what survives: what the plan says goes and is still standing, older code
   in the area the change made redundant, and what an earlier pass built
   that a later one superseded. Never repair or extend code the plan
   deletes. Keep required behavior, data and tests on the surviving path,
   with any consumer cutover in the same cut. If the cut breaks something,
   finish the cut.

3. **List what the branch added and question each entry.** Review every type,
   function, field, parameter, flag, config key and test. Look for additions
   only tests use or nobody calls, wrappers that only forward and pairs that
   do the same job. Delete, inline or merge. When every caller uses the same
   value for a parameter, make that value the behavior.

4. **Then look at every scale in what remains:** names, control flow, function
   boundaries, a more direct algorithm or data structure, repeated work, extra
   concepts and dependencies, and the tests, config and docs around them.

5. **Choose the shape that makes the problem simplest,** even when it moves
   several files or rewrites what several passes built. Extract or inline,
   combine or split, reuse what exists. Remove indirection, state and
   speculative flexibility; keep abstractions that help a reader. Clarity
   beats line count.

6. **Check, then commit, each cut.** When a cut changes code, build the
   narrowest changed target or run its focused test; skip the check for
   prose-only edits, and don't rerun without an edit. Skip release builds,
   workspace-wide linters and full suites — CI runs them. Commit the
   cut on its own with what it removed and why, so one can be undone
   without the others.

7. **Rewrite the reading order** when `scratch/tour.yaml` exists, as
   `lf help tour` describes: your cuts moved what a reader should open first.

What people see, the data they have and a number the branch was built to
move stay as they are. Defer cuts that would change any of those. When
`scratch/plan.md` exists, record each deferred cut and what it would take
under `## Deferred findings`, and put questions only a person can answer in
`scratch/questions.md`, creating it if needed. Without a plan, put deferred
cuts in your reply, and questions in `scratch/questions.md` if it exists or
in your reply otherwise.

Keep the reason for a non-obvious cut beside the code. If nothing needs
reducing, say so and stop.
