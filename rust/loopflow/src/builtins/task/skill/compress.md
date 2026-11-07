---
requires: branch diff and current design
produces: simpler code related to the diff
action_style: procedural
---
Simplify anything related to the branch diff so the code is easier to understand
and change, without losing required behavior. The output is a diff.

1. Read the full diff and design, then follow the change into its callers,
   tests, configuration and docs. Scope is everything related to the change, not
   just modified lines. Take every worthwhile simplification there; don't stop
   after one small win or wander into unrelated cleanup.

2. Make the deepest cuts first. Read or update the plan's **Delete — do not
   maintain** list of files and symbols and their exclusive tests and fixtures.
   Remove obsolete concepts and paths before polishing what survives. Never
   repair, refactor or extend code the plan removes. Keep required behavior,
   data and tests on the surviving path, with any consumer cutover or migration
   in the same cut. If the cut breaks something, finish the cut; don't repair
   the predecessor. Keep the remaining deletion targets current. Additive work
   needs no deletion.

3. Then look at every scale in what remains: names, control flow, function
   boundaries, a more direct algorithm or data structure, repeated work, extra
   concepts or dependencies, and the tests, config and docs around them.

4. Choose the shape that makes the problem simplest. Extract or inline, combine
   or split, reuse what already exists. Remove indirection, state and speculative
   flexibility; keep abstractions that help readers. Clarity beats line count.

5. Build changed code and run its focused test when behavior changed. No edits
   means no rerun. Fix real failures. A check that can't run headless goes to
   gate or CI; don't block the Graph on it. Ask for a judgment you need when
   someone is in the conversation; headless, leave it for demo. Gate runs the
   affected suites.

Keep rationale beside the code or in the plan, plus one command/result line.
If nothing needs reducing, say so. Realign reconciles the plan.

## Keep authored context within budget

Run `lf context` for memory and scratch size targets, where they are configured
and current usage. Read sources in full before curating them, and bring anything
over budget under both the token and byte limits. Keep what can't be worked out
again from the code, git history or a fresh run: what people said, word for
word, what a run showed at the time, facts from outside the repo, plus
open questions and contrary findings. Cut the rest: merge duplicates, drop
obsolete notes inherited from a stacked parent, and leave history in git with a
precise reference when older detail still matters. Quality per word, not
volume. Commit or move uncommitted notes before removing them. Edit existing
notes rather than adding reports. Re-run `lf context` after writing. Never raise
limits to hide overflow; if the live decisions alone don't fit, record the
conflict and the remaining overage.
