# Prompts

You are running one prompt in a larger system. Other prompts may run before or after you. This doc describes the full suite so you understand what state you inherit and what state to leave.

Principle: tight loops. Each prompt does one thing. Make progress and hand off cleanly.

---

## The Prompts

| Prompt | Requires | Produces |
|--------|----------|----------|
| design | — | .design/<branch>.md |
| implement | .design/<branch>.md | code, tests |
| polish | code on branch | passing tests, .design/ updated |
| review | code on branch | verdict in .design/ |
| iterate | diff vs main | improved code |
| reduce | diff vs main | simplified code |
| expand | diff vs main | .design/<expansion>.md |
| draft_commit | changes | .lf/COMMIT |
| rebase | — | rebased branch |
| debug | — | fixed code |
| refine | — | refined text |

**Requires** = what must exist before you run. If it's missing, stop and report.

**Produces** = what you leave for the next prompt. Make sure this exists when you finish.

---

## Common Sequences

These show how prompts chain. Understand where you fit.

```
design → implement → polish → draft_commit
```
Design writes the spec. Implement reads it and writes code. Polish verifies and cleans up. Draft_commit summarizes.

```
polish → draft_commit
```
Human coded a fix. Polish verifies it. Draft_commit summarizes.

```
iterate → iterate → polish
```
Each iterate makes one improvement. Polish proves nothing broke.

```
review → iterate → polish
```
Review finds issues. Iterate fixes them. Polish closes it out.

```
expand → design → implement
```
Expand writes a speculative .design/ doc. If worth building, design refines it, then implement builds it.

---

## What You Inherit

Check these before starting:

- **Branch**: Are you on a feature branch or main? Some prompts create branches; others expect one.
- **.design/<branch>.md**: Does a design doc exist? Implement requires it. Polish updates it.
- **Code on branch**: Is there a diff vs main? Iterate, reduce, expand require changes to work with.
- **Tests**: What's the current test state? Polish needs to leave them passing.

---

## What You Leave

Before finishing:

- **Don't commit unless you're draft_commit.** Other prompts leave changes uncommitted.
- **Update .design/ if you changed the implementation.** Polish does this; implement doesn't.
- **Note open questions in .design/questions.md.** Don't block on unknowns—capture them and move on.
- **Leave tests passing.** If you break tests and can't fix them, stop and report.

---

## Quality Principles

Apply these to all code you write or modify:

- **Types over prose** — Skip Args:/Returns: docstrings if types tell the story
- **Results over mocks** — Tests assert on behavior, not mock calls
- **One version** — No `_v2`, `_old`. Delete the old.
- **Delete over deprecate** — Unused code gets removed, not commented
- **Functions over classes** — Use classes only when you need state

---

## Auto Mode

In auto/headless execution:

- Don't pause for questions
- Make best assumptions
- Write open questions to `.design/questions.md`
- Keep moving

Questions don't block the loop. They get captured for the next pass.
