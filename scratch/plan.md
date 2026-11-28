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
- **Nothing gets built on top afterwards.** A reshape no longer risks being
  undone by the next slice, so a cut that moves several files is safe to take.
- **`qa` uses the real thing next, then a person reads `ship-review`.** A cut
  that breaks behavior is caught by the next step, and each step's commits
  are what `ship-review` reads (`ship-review.md`: "`git log` against the base:
  each step's commit says what that step changed").

## The design

One file changes its meaning: `compress.md`. Choices below are made for
review; nobody was in the conversation.

1. **Scope is the whole change in the area it lands in**, read against the
   base as one thing, as someone seeing it for the first time. This borrows
   the framing `concept-review` and `deep-clean` already use. Passed over:
   keeping "whatever the change reaches" (too timid for a once-per-Task step)
   and widening to the codebase (that is `reduce` and `deep-clean`, which
   file or take their own Tasks).

2. **Look for what only the whole shows.** Seams between passes; older code
   in the area that the finished change made redundant; anything on the
   plan's "What goes" list still standing.

3. **Work a list of what the branch added**, not an impression. Every type,
   function, field, parameter, flag, config key and test the branch
   introduced, checked for `deep-clean`'s flags: nothing in production calls
   it, only tests use it, every caller passes the same value, a wrapper that
   only forwards, two definitions doing one job. Unlike `deep-clean` the list
   is not kept in `scratch/`; it is a working method for one run. Passed
   over: leaving step 3 ("look at every scale") as the only instruction.
   It names what to look at but gives no way to know the look was complete.

4. **Take the reshape, not only the trim.** When a simpler data structure or
   one path instead of two would carry the whole change, make that change
   even though it rewrites what several passes built. The limit is the one
   the skill already has: behavior people rely on and their data stay. A cut
   that would change what people see, or that needs something only a person
   knows, is not taken; it goes under `## Deferred findings` in
   `scratch/plan.md`, the section `ship-review` already reads, with what it
   would take. Passed over: letting compress return `iterate` to send a
   design problem back to `build`. It is outside the loop in both Graphs,
   and adding a loop around it reopens the thin-pass cost #2037 removed.

5. **One commit per cut, with the reason.** Each is reversible on its own at
   `ship-review`. The runner commits leftovers at step end, so this is about
   granularity, not about remembering to commit.

6. **Checks stay narrow.** A focused test per cut that changed behavior; CI
   runs the suites. Being more ambitious about cuts does not buy wider local
   test runs on a shared Machine (AGENTS.md "When to run what").

7. **Close with an account**: what went, what was kept and why, and lines
   against the base before and after. A sentence in the reply, not a file.
   "Clarity beats line count" stays, so the number is reported, not required
   to fall.

8. **In `optimize`, the measured number is behavior.** One clause, so a
   reshape doesn't trade away the win the Graph exists for; its final
   `measure` step would show the loss, but only after the fact.

Sentences from the current text that still hold stay word for word; it was
settled in the 2026-11-16 refine sitting (#1955). Voice follows that sitting:
bold imperatives, at most one "The test:".

### Draft text

```markdown
---
requires: the finished change against its base, and the plan
produces: a simpler change and a simpler area around it, in commits that say what each cut removed
action_style: procedural
---
Simplify everything this branch touches until it is easy to understand and change, keeping the behavior people rely on. The output is a diff.

This runs once, after the design has settled and before anyone uses the
change. Nothing will be built on top of it later, so take the cut that moves
several files as readily as the one that renames a variable.

Scope is the whole change and the area it lands in: its callers, tests,
configuration and docs, not just the modified lines. Take every worthwhile
simplification there. Don't stop after one small win, and don't wander into
unrelated cleanup.

1. **Read the whole change against its base, and the plan,** as someone
   seeing it for the first time. It was built in passes, each with its own
   context; you are the first to see it whole.

2. **Cut deepest first.** Remove obsolete concepts and paths before polishing
   what survives: what the plan says goes and is still standing, older code
   in the area the finished change made redundant, and what an earlier pass
   built that a later one superseded. Never repair or extend code the plan
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

6. **Commit each cut on its own,** with what it removed and why, so one can
   be undone without the others.

7. **Check.** Check only the narrowest changed target or focused test when
   behavior changed. No edits, no rerun. Skip release builds, workspace-wide
   linters and full suites — CI runs them.

8. **Rewrite the reading order** when `scratch/tour.yaml` exists, as
   `lf help tour` describes: your cuts moved what a reader should open first.

What people see and the data they have stay as they are, and so does a
number the branch was built to move. A cut that would change one of those, or
that needs something only a person knows, is not yours to take: write it
under `## Deferred findings` in `scratch/plan.md` with what it would take.

Keep the reason for a non-obvious cut beside the code. Finish with what went,
what you kept and why, and the change's size against the base before and
after. If nothing needs reducing, say so and stop.
```

About 430 words, up from 274; `implement` is 959 and `realign` 822.

## Affected surfaces

- `rust/loopflow-definitions/src/builtins/task/skill/compress.md`: the text
  above. Embedded at build time; nothing pins its wording (searched Rust,
  Python and Swift tests for its phrases).
- `docs/orchestration.md:155`: the `build` listing's comment "simplify what
  the branch touched" becomes "simplify the finished change as a whole".
  Line 13 ("`compress` simplifies") and line 145 already fit.
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
- *`## Deferred findings` gets a second writer.* `realign` replaces settled
  review notes "while keeping `## Deferred findings` intact"
  (`realign.md`), so an entry compress adds survives a later realign.

## Pass

One pass, prose only.

1. Replace `compress.md` with the draft; edit the one comment in
   `docs/orchestration.md`.
2. Check: `cargo build -p loopflow` then `target/debug/lf help compress`
   prints the new text (the builtin is `include_str!`'d, so a build is what
   shows it parses and registers). No test run: no behavior with a test
   changed.

## Demo

`lf help compress` shows the new skill. The felt difference is on the next
multi-pass Task: after `build`, `git log` shows compress's commits as
separate cuts with reasons (cross-pass duplicates merged, single-use
parameters gone), and `ship-review` lists anything it deferred.

## Done when

- `lf help compress` describes a step that reads the whole change against
  its base, lists what the branch added, commits each cut, and defers cuts
  that change what people see.
- `docs/orchestration.md` describes the same step.
