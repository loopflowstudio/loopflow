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
| debug | — | fixed code |

**Requires** = what must exist before you run. If it's missing, stop and report.

**Produces** = what you leave for the next prompt. Make sure this exists when you finish.

---

## Common Sequences

These show how prompts chain. Understand where you fit.

```
design → implement → polish
```
Design writes the spec. Implement reads it and writes code. Polish verifies and cleans up.

```
polish
```
Human coded a fix. Polish verifies it.

```
iterate → iterate → polish
```
Each iterate makes one improvement. Polish proves nothing broke.

```
review → iterate → polish
```
Review finds issues. Iterate fixes them. Polish closes it out.

---

## What You Inherit

Check these before starting:

- **Branch**: Are you on a feature branch or main? Some prompts create branches; others expect one.
- **.design/<branch>.md**: Does a design doc exist? Implement requires it. Polish updates it.
- **Code on branch**: Is there a diff vs main? Iterate requires changes to work with.
- **Tests**: What's the current test state? Polish needs to leave them passing.

---

## What You Leave

Before finishing:

- **Don't commit unless explicitly asked.** Other prompts leave changes uncommitted.
- **Update .design/ if you changed the implementation.** Polish does this; implement doesn't.
- **Note open questions in .design/questions.md.** Don't block on unknowns—capture them and move on.
- **Leave tests passing.** If you break tests and can't fix them, stop and report.

---

## Auto Mode

In auto/headless execution:

- Don't pause for questions
- Make best assumptions
- Write open questions to `.design/questions.md`
- Keep moving

Questions don't block the loop. They get captured for the next pass.
