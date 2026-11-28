---
requires: the branch diff against its base, and the plan if one exists
produces: a simpler change, with one commit per cut
action_style: procedural
---
Simplify everything this branch touches until it is easy to understand and change, keeping the behavior people rely on. The output is a diff.

1. **Read the branch against its base as one change.** Read the plan if one
   exists. Follow the change through its callers, tests, configuration and
   docs. Take every worthwhile simplification there.

2. **Cut deepest first.** Remove what the plan retires, what the change made
   redundant, and what a later pass replaced. Move callers, data and tests
   to the surviving path in the same cut. If the cut breaks something,
   finish the cut.

3. **List what the branch added.** For each type, function, field, parameter,
   flag, config key and test, look at who uses it. Merge duplicates across
   passes, inline wrappers that only forward, and drop parameters whose
   callers all pass the same value. Remove unused code and production code
   kept alive only by tests.

4. **Choose the shape that makes the problem simplest.** Follow a clearer
   algorithm or data structure across files, even if it rewrites several
   passes. In what survives, simplify names, control flow and function
   boundaries. Remove repeated work, needless state and dependencies.
   Clarity beats line count.

5. **Check each cut before committing it.** For code, build the narrowest
   changed target or run its focused test. Skip builds for prose and reruns
   without an edit. Leave release builds, workspace-wide linters and full
   suites to CI. Commit each cut separately, saying what it removed and why.

6. **Rewrite the reading order** when `scratch/tour.yaml` exists, as
   `lf help tour` describes.

Defer cuts that would change visible behavior, people's data or a
measured result the branch was built to achieve. Record each cut and what
it would take under `## Deferred findings` in `scratch/plan.md`. Put questions
only a person can answer in `scratch/questions.md`. Without a plan, report
deferred cuts in your reply and include questions there unless the questions
file already exists.

Keep the reason for a non-obvious cut beside the code. If nothing needs
reducing, say so and stop.
