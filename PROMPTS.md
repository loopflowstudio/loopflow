---
layout: default
title: Prompt Authoring
---

# Prompt Guide

How to write Skills, Wave goals and Task briefs for Loopflow. The same words
should read well to a person and work for an agent.

This guide is for people writing prompts: maintainers editing builtins, and
anyone writing `.lf/skills/*.md` or `.lf/wave/<name>/GOAL.md`. It is not loaded
into ordinary runs. `lf prompt` carries a short version of it.

## Write down what can't be recovered

The intelligence available later will be at least what's available now.
Anything that can be worked out again from the code, the history or a fresh run
can be worked out again, so it rarely needs to be written down. What can't be
recovered is worth capturing exactly:

- a person's exact words
- what a run showed at the time
- facts from outside the repository

Plans still help: people and agents both work by reasoning out what to do,
writing it down, and letting it guide them. But more words is not better.
Optimize for quality per word.

This applies to prompts themselves. Every line a Skill adds is read on every run
that uses it, and every repeated word loses force. Keep only what changes what
the agent does.

## Where an instruction goes

Put each instruction in the narrowest place that uses it:

| Place | What belongs there |
| --- | --- |
| A Skill | One repeatable job and how to do it |
| A Graph | The order of Skills, and where a person takes part |
| Wave `GOAL.md` | Why the Wave exists, what it owns and refuses, how it picks work |
| Wave `MEMORY.md` | Decisions and lessons that change future work |
| `AGENTS.md` | Rules every task in this repository follows |
| `LOOPFLOW.md` | The few things every Loopflow run needs |
| A Task brief | One problem and what done looks like |

Builtin Skills ship to everyone and run in repositories without Loopflow's
files, so they carry their own method. They are a starting point: people copy
them with `lf skill customize <name>` and make them their own. A repository's
`.lf/skills/<name>.md` replaces the builtin of the same name.

## Shape of a Skill

```markdown
---
requires: diff vs main
produces: simpler code
---
Simplify code touched by this branch while keeping what users rely on.
```

Frontmatter says what the Skill needs and what it leaves behind. The first line
says what it does, in one sentence. No preamble, no "This prompt will…".

Then the steps, with real commands and file paths. Add a section on what to
leave alone when the agent would otherwise wander. End with what it should hand
back, if that isn't obvious.

Write a Skill as one useful operation, not as a stage of one particular Graph.
It should work alone, in the middle of other work, and when run twice. Graphs
compose Skills; publishing, landing and choosing the next step belong to the
Graph or the person, not the Skill.

Interactive and headless runs use the same Skill. Mark an `interactive:` step
in the Graph rather than writing two modes into the Skill, unless the job
really differs.

## Voice

**Direct and imperative.** "Run tests." "Read the diff." Not "You should run
tests."

**No identity framing.** Don't tell the agent what it is ("You are a code
reviewer…"). Just say what to do.

**Concrete.** Name the command, the file and what a person sees. If a sentence
can't be restated as an action or a fact, cut it.

**Opinionated.** If there's a right way, say so, in one line: "The deliverable
is the fixes themselves, not a written review." "Slow is a bug."

**Real use over checks.** Ask the agent to use the change the way people will,
and to show what it saw. Don't ask for "proof" as a thing to produce.

**Describe the work, not who asked for it.** Skills, docs, PRs and Task briefs
say what the system does and what the work is. No "Jack said", no "the user".
Exact quotes with names belong in `scratch/` and design notes.

**Quote, don't rule.** Record what people said, not a verdict about it. A
"looks fine" after a long plan is not a decision; never write it up as
"accepted" or "approved". Ask specific questions, numbered so "2b" is a complete
answer, and quote the answer.

**Spend words carefully.** Every repeated word loses force. Watch for filler
like "evidence", "preserve", "explicit", "boundary", "durable",
"consequential", "ensure", "appropriate".

**Vary it.** A Skill that runs often should let each run respond to what's
actually there, not follow a script that produces the same output every time.

## Task briefs and comments

A Task brief says what's wrong or missing, who it affects and what done looks
like. A short paragraph and a few bullets are usually enough. Possible
solutions are marked as ideas. Keep exact numbers and failure conditions.

```markdown
After an interrupted release, it's unclear whether anything shipped or how to
continue safely.

- Retrying unfinished work does not publish the same release twice.
- A failed check stays visible and stops publication.
```

Comments hold what changed since: progress, attempts, new constraints. When a
comment changes the accepted work, update the brief too; a reader shouldn't
have to replay the comments to know what's being built.

## PRs and reports

Write for someone coming back after time away. Lead with what's better now:
"Switch Tasks without losing unfinished input", not "Persist terminal state per
Task". Then what changed, briefly. Test, lint and QA results never go in a PR
body; CI shows them on the PR. A check that can only run after merge stays:
what is untried, the environment it needs and what to look for. A report says
what changed, what needs attention and the next decision.

## Wave goals

A Wave's `GOAL.md` is a goal run in a loop. Five things make one loop well:

1. **Identity by contrast.** What the Wave is and isn't, ideally against a
   sibling: "Systems owns the machinery around the code; Architecture owns the
   shape of the code."
2. **Signals it can read,** most important first: `green on main`, `test time`,
   `net concept count flat-or-falling`. If a loop can't read it, it can't steer.
3. **Concrete moves.** "Sand a sharp edge, automate a manual ritual, harden a
   flaky pipeline, or turn a failure into a fix PR." "Make it better" is
   unactionable at 2 a.m.
4. **An honest question that resists gaming:** "the honest question is never
   *how much did you delete*; it is *did a design ship and is the tree lighter
   a quarter later*."
5. **A stop rule.** "If no safe move remains, record the blocker instead of
   inventing work."

Goals give judgment, not procedure. "Where's the biggest gap between vision and
implementation?" is a goal; a numbered checklist belongs in a Graph. Loopflow
concepts (`.lf/wave/`, `scratch/`, Graphs) are fine in goals; product details
("44pt tap targets") are not.

Projects are measured bets inside one Wave. Start from who benefits and what
changes for them. A KR is an end state that would show the bet holds, not a
task list.

## Opening lines

Work:
- "Does this work? Ship or iterate?"
- "Simplify code touched by this branch while keeping what users rely on."
- "Look at this codebase through its users' eyes."

Don't work:
- "You are a code reviewer who…"
- "This prompt helps with…"
- "Please review the code and…"

## Before committing a prompt

- [ ] Frontmatter has `requires:` and `produces:`
- [ ] The first line says what it does
- [ ] Steps are concrete: commands, paths, what a person sees
- [ ] Every line changes what the agent does
- [ ] No names or "the user" in anything meant to last
- [ ] It works alone, mid-stream, and when run twice
- [ ] Anything it writes down is something that couldn't be recovered later
