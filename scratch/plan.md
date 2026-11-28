# LOO-1127: a more ambitious `compress`

Brief, word for word: "lets refine compress to be a little more ambitious now
that it only runs once per task rather than once per slice".

## What changed around it

`compress` was written for a step that ran inside the build loop, once per
slice, on a small diff, with more slices still to come. Since #2047
(`6d617de686`) `build.yaml` runs it once, after the implement loop and the
concept-review/realign loop have both settled, with `qa` straight after:

```yaml
- loop:
    - loop: [sync, implement]
    - concept-review
    - realign
- compress
- qa
- pr-publish
```

`optimize.yaml` also runs it once, after its loop. No other builtin Graph
uses it, and this repository has no `.lf/skills/compress.md`.

The skill text (`rust/loopflow-definitions/src/builtins/task/skill/compress.md`,
274 words) still reads like the per-slice step: scope is "whatever the change
reaches", and it warns against wandering. Three things are now true that the
text doesn't use:

- **It sees the finished change.** Passes were built by separate contexts.
  Their seams (a helper written twice, pass-one scaffolding that pass three
  superseded, an adapter between two passes' shapes) are invisible to any one
  `implement` run and to `realign`'s "one look at the shape".
- **No slice follows it in the loop.** A reshape no longer risks being
  undone by the next slice, so a cut that moves several files is safe to take.
  (A `ship-review` loop back to `build` runs it again; that's fine.)
- **`qa` uses the real thing next, then a person reads `ship-review`.** A cut
  that breaks behavior is caught by the next step, and each step's commits
  are what `ship-review` reads (`ship-review.md`: "`git log` against the base:
  each step's commit says what that step changed").

## The design

Two builtin files change: `compress.md` (its meaning) and one line of
`ship-review.md`. Every choice below is made for review; nobody was in the
conversation. The concept review (`scratch/concept-review.html`) reshaped
the first draft; what happened to each finding is under "Review findings".

The schedule is why more ambition pays now, but the skill must not assert it:
`prompt.md:46` asks for "one useful operation, not a stage of one Graph", and
compress also runs in `optimize` (before `refresh`), after a `ship-review`
loop back to `build`, and as plain `lf compress` mid-branch. The skill says
what the operation is (one change against its base) and lets the schedule
stay in the Graph.

1. **Scope is the whole change in the area it lands in**, read against the
   base as one change, not a sequence of edits. Passed over: keeping
   "whatever the change reaches" (too timid) and the codebase (`reduce`,
   `deep-clean`).

2. **Look for what only the whole shows.** Seams between passes; older code
   in the area that the change made redundant; anything on the plan's "What
   goes" list still standing.

3. **Work a list of what the branch added**, checked for `deep-clean`'s flags
   (no production caller, only tests use it, every caller passes the same
   value, a forwarding wrapper, two doing one job). An in-run method; no
   `scratch/` worklist. Passed over: "look at every scale" alone, which gives
   no way to know the look was complete.

4. **Take the reshape, not only the trim.** Behavior people rely on, their
   data, and a number the branch was built to move (`optimize`) stay. A cut
   that changes one of those goes under `## Deferred findings` in
   `scratch/plan.md` when a plan exists, with what it would take; a question
   only a person can answer goes in `scratch/questions.md`; with neither file,
   both go in the reply. `ship-review.md:14` drops "written by `realign`".
   Passed over: keeping compress out of `scratch/plan.md` (questions only),
   which takes concrete proposals off `ship-review`'s Fix here / File Task /
   Drop cards; and returning `iterate` to `build`, which reopens the
   thin-pass cost #2037 removed.

5. **Check, then commit, each cut.** One step: the narrowest check when
   behavior changed, then a commit saying what it removed and why, so one cut
   reverts without the others. Passed over: commit-then-check, which commits
   broken cuts and adds fixup commits.

6. **Checks stay narrow**; CI runs the suites.

7. **No closing report.** Commit messages and code comments carry the
   reasons; `git diff --stat` carries the size. Passed over: a "what went,
   what was kept, lines before and after" reply that headless `build` logs
   and nobody reads.

Sentences from the current text that still hold stay word for word (settled
in the 2026-11-16 refine sitting, #1955). Voice: bold imperatives, at most
one "The test:".

### Draft text

```markdown
---
requires: the branch's changes against its base, and the plan if one exists
produces: a simpler change and a simpler area around it, in commits that say what each cut removed
action_style: procedural
---
Simplify everything this branch touches until it is easy to understand and change, keeping the behavior people rely on. The output is a diff.

Treat the branch as one change against its base, not a sequence of edits.
Take a reshape that moves several files as readily as one that renames a
variable.

Scope is the whole change and the area it lands in: its callers, tests,
configuration and docs, not just the modified lines. Take every worthwhile
simplification there. Don't stop after one small win, and don't wander into
unrelated cleanup.

1. **Read the whole change against its base, and the plan,** as someone
   seeing it for the first time. Passes built by separate contexts leave
   seams that only the whole shows.

2. **Cut deepest first.** Remove obsolete concepts and paths before polishing
   what survives: what the plan says goes and is still standing, older code
   in the area the change made redundant, and what an earlier pass built
   that a later one superseded. Never repair or extend code the plan
   deletes. Keep required behavior, data and tests on the surviving path,
   with any consumer cutover in the same cut. If the cut breaks something,
   finish the cut.

3. **List what the branch added and question each entry:** every type,
   function, field, parameter, flag, config key and test. Nothing in
   production calls it; only tests use it; every caller passes the same
   value; it only forwards; two of them do one job. Delete, inline or merge.
   A parameter every caller leaves alone becomes the behavior.

4. **Then look at every scale in what remains:** names, control flow, function
   boundaries, a more direct algorithm or data structure, repeated work, extra
   concepts and dependencies, and the tests, config and docs around them.

5. **Choose the shape that makes the problem simplest,** even when it
   rewrites what several passes built. Extract or inline, combine or split,
   reuse what exists. Remove indirection, state and speculative flexibility;
   keep abstractions that help a reader. Clarity beats line count.

6. **Check, then commit, each cut.** Check only the narrowest changed target
   or focused test when behavior changed. No edits, no rerun. Skip release
   builds, workspace-wide linters and full suites — CI runs them. Commit the
   cut on its own with what it removed and why, so one can be undone
   without the others.

7. **Rewrite the reading order** when `scratch/tour.yaml` exists, as
   `lf help tour` describes: your cuts moved what a reader should open first.

What people see, the data they have and a number the branch was built to
move stay as they are. A cut that would change one of those is not yours to
take: write it under `## Deferred findings` in `scratch/plan.md` with what it
would take. Put a question only a person can answer in
`scratch/questions.md`. Without those files, say both in your reply.

Keep the reason for a non-obvious cut beside the code. If nothing needs
reducing, say so and stop.
```

## Affected surfaces

- `rust/loopflow-definitions/src/builtins/task/skill/compress.md`: the text
  above. Embedded at build time; nothing pins its wording (searched Rust,
  Python and Swift tests for its phrases).
- `rust/loopflow-definitions/src/builtins/task/skill/ship-review.md:13-14`:
  "and `## Deferred findings` written by `realign`." becomes "and
  `## Deferred findings`." (realign and compress both write it;
  `realign.md:67` keeps the section intact across refreshes).
- `docs/orchestration.md`: updated in place (`write-docs`) — line 155 comment
  changed to "simplify the change as a whole" and a paragraph added under
  `### Loops` covering whole-branch scope, per-cut commits, and deferred
  findings / standalone `lf compress`. Line 13 ("`compress` simplifies") and
  line 145 already fit.
- `build.yaml` and `optimize.yaml`: unchanged. Their descriptions and
  comments already say compress runs once.
- Repositories with a `.lf/skills/compress.md` that `extends: lf:compress`
  get the new base with their additions; a `--full` copy is theirs and does
  not move. No migration (builtins are a starting point people copy).
- `AGENTS.md:457` and `.github/CI.md:15` ("implement / compress: build the
  changed code and run its focused test") still describe the check.

## Exclusions

- No Graph change: compress stays outside the loop and decides nothing.
- No new settings (agent, effort) for compress.
- `deep-clean` and `reduce` keep their text; compress borrows their flags,
  it doesn't replace them.
- `quick.yaml` has no compress step and stays that way: a small, clear
  change has no passes to reconcile.

## What goes

- The old `requires`/`produces` lines ("branch diff and current design",
  "simpler code related to the diff").
- "Scope is whatever the change reaches".
- Nothing else; no code, tests or fixtures are tied to the old wording.

## Risk, answered

- *A bolder compress breaks behavior after review settled.* `qa` runs next on
  the real thing and `ship-review` reads the step's commits; per-cut commits
  make one bad cut revertible. The deferral rule keeps visible behavior and
  data out of its reach.
- *It turns into `deep-clean` across the codebase.* Scope is still the area
  the change lands in, and "don't wander into unrelated cleanup" stays.
- *Longer runs.* It is one step per Task where it used to be one per slice;
  the per-Task total falls even if this run doubles.
- *`## Deferred findings` gets a second writer.* `realign` keeps the section
  intact when it replaces review notes (`realign.md:67`), and `ship-review`
  stops naming a single writer.
- *A bolder cut in `optimize` loses the speedup.* The skill keeps the
  measured number; the Graph's recovery when it slips is deferred below.

## Docs written

- `docs/orchestration.md`

## Pass

One pass, prose only. Nothing is built yet (`docs/orchestration.md` is updated).

1. Replace `compress.md` with the draft and edit the `ship-review.md` line
   (reading `docs/orchestration.md` for any edits made since).
2. Check: `cargo build -p loopflow` then `target/debug/lf help compress`
   prints the new text (the builtin is `include_str!`'d). No test run.

It teaches whether the draft reads as one operation when shown alone by
`lf help compress`, without the Graph around it.

## Demo

`lf help compress` shows the new skill. The felt difference is on the next
multi-pass Task: after `build`, `git log` shows compress's commits as
separate cuts with reasons (cross-pass duplicates merged, single-use
parameters gone), and `ship-review` lists anything it deferred.

## Done when

- `lf help compress` describes a step that reads the whole change against
  its base, lists what the branch added, checks then commits each cut, and
  defers cuts that change what people see, their data or a measured number,
  without claiming where in a Graph it runs.
- `ship-review.md` names no single writer of `## Deferred findings`.
- `docs/orchestration.md` describes the same step.

## Review findings (concept review, plan stage)

- *Skill asserted `build`'s schedule* ("This runs once… Nothing will be
  built on top of it later"): fixed in the draft; opening now says what the
  operation is, `requires:` makes the plan optional.
- *Second writer on `## Deferred findings`, mixed with open questions, no
  plan in standalone runs*: adopted the review's recommendation (its
  question 1a), made for review: cuts to the plan when one exists, questions
  to `scratch/questions.md`, otherwise the reply; `ship-review.md:14` edit
  added to the pass.
- *Commit before check*: fixed; one check-then-commit step.
- *Closing report nobody reads*: dropped from the draft.
- *Stay outside the loop* (its question 2a): already the plan.
- *`optimize` rolls back every compress cut on a final-measure regression*:
  deferred below.

## Deferred findings

- **`optimize`'s final `measure` discards all compress cuts together.**
  `optimize.yaml` runs compress → refresh → measure (final); on regression
  `measure.md:41` reverts to the last kept commit, dropping every cut and the
  refresh at once. Would take one small Task on `optimize.yaml` and
  `measure.md`: measure right after compress, before refresh, or a hot-path
  check rule when `scratch/measure.html` exists. Worth filing only if it
  happens in practice.
