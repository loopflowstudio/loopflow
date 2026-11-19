---
description: Write or audit a Loopflow skill, Wave goal, or inline prompt.
requires: a prompt idea or existing prompt
produces: .lf/skills/*.md | .lf/wave/<name>/GOAL.md | reviewed prompt text
action_style: exploratory
---
Turn intent into a prompt that steers real work and knows when it is done.

```bash
lf prompt: create a dependency-audit skill
lf prompt: tighten .lf/wave/infra/GOAL.md
```

Read the intent and the existing file. Ask only about choices that change
behavior or scope, and edit as decisions land. Headless, make the corrections
you can back up and leave real ambiguity visible.

Every line of a prompt is read on every run that uses it, and every repeated
word loses force. Keep only what changes what the agent does. Today's models
need judgment and a point of view, not a procedure for every case.

## Pick the layer

Put each instruction in the narrowest place that uses it:

| Layer | Holds | Not |
| --- | --- | --- |
| Skill | One repeatable job and how to do it | Repo conventions, Wave policy |
| Graph | The order of Skills and where a person takes part | How to do a step |
| Wave `GOAL.md` | What the Wave owns and refuses, how it picks work | Task lists, implementation steps |
| Wave `MEMORY.md` | Decisions and lessons that change future work | A second plan, a transcript |
| Agent guide | Rules every task in the repository follows | One feature's design |
| Inline prompt | One concrete request | Guidance that deserves a skill |

A shared skill runs in other people's repositories: no source paths, issue IDs
or local policy. A repository's `.lf/skills/<name>.md` replaces the builtin of
the same name; it doesn't append to it.

## Write it

1. **Define done.** The exact job, what success looks like when someone uses
   the result, the near-misses that don't count, and the command or
   observation that shows it worked. Name who reads the output and what they
   decide next.

2. **Make it one useful operation,** not a stage of one Graph. Ask for the
   information the job needs, not the skill that happened to supply it once.
   It works alone, in the middle of other work, and when run twice; on work
   already done it stops without manufacturing edits. Publishing, landing and
   choosing the next step belong to the Graph.

3. **Say it straight.** Frontmatter with `requires:` and `produces:`, then one
   sentence that says what it does. Imperative, concrete: the command, the
   file, what a person sees. No "You are a…", no "This prompt will…". Where
   there is a right way, say so in one line: "The deliverable is the fixes
   themselves, not a written review." Procedural skills get numbered steps;
   exploratory skills get room to follow what they find.

4. **Give uncertain work a feedback loop.** Keep observations apart from
   hypotheses, run the smallest check that separates the leading explanations,
   and replan after a counterexample. A skill that writes notes keeps only what
   can't be worked out again: what people said, word for word, what a run
   showed, outside facts. A reply is never a verdict; ask numbered questions so
   "2b" is a complete answer.

5. **Audit it as a lazy agent.** Could it satisfy the words and miss the
   point? Does "tests added" pass for the outcome? Does it know what to do when
   findings contradict the plan? Close the easy loopholes, with a tool or a
   test where prose can't hold the line.

6. **Edit at the source.** Update the named file or return the text; no second
   copy in docs. Summarize the change in behavior, not each wording edit.

## Wave goals

A `GOAL.md` body is a prompt run over and over. Five things make it loop well:

1. **Identity by contrast** — what this Wave owns and what a sibling owns.
2. **Signals it can read** — if a loop can't read it, it can't steer by it.
3. **Concrete moves** — the kinds of action it can take now.
4. **An honest question** — the check a lazy loop can't game.
5. **A stop rule** — when to record a blocker instead of inventing work.

Goals give judgment, not procedure; implementation lives in Tasks.
