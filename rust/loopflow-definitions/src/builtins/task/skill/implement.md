---
requires: intended change and an implementation plan
produces: working code, a focused check, and an updated plan
action_style: procedural
---
Turn the plan into working code.

Working code with rough edges beats perfect code that took too long. A first
draft doesn't satisfy acceptance it hasn't met.

Read the plan in `scratch/` and the agent guide; a small change may have its
plan in the conversation. Read the affected user docs too, including edits made
since the last pass: they describe the experience to build, so keep their
corrections.

## Build

- **Deepest deletions first.** Remove what this change replaces, then build on
  what remains, in the same diff. Never repair, refactor or extend code the
  plan deletes; when its test fails, delete it and cover the behavior on the
  surviving path. A broken build mid-cut is no reason to fix the old path:
  finish the cut, then check. New capabilities need no deletion.
- **One concept, one representation.** A new type means a new real-world
  concept. Reshape the code that exists. No flags, v2 types, adapters,
  fallbacks, dual writes or setup nothing uses yet, unless the plan calls for
  one and says when it is deleted.
- **Move real consumers end to end**, with any data migration in the same cut.
- **Match what's there.** If the codebase uses `@dataclass`, use `@dataclass`.
- **Stay in scope.** Build what the plan describes. Other ideas go in
  `scratch/questions.md`, not the code.
- **Write for the load it will carry.** Take data size and concurrency from
  what the repository says it serves, not from the fixture. Read the plan of
  every query you add or change, bound what a hot path reads, keep writes
  short, and time it at realistic size.
- **Test behavior.** Assert on what people see, not on mock calls.

## Check

Build the changed code and run the focused test for the changed behavior: the
narrowest filter that names it, never a whole package. One build at a time.
Skip builds for prose-only edits, and don't rerun a passing command without a
change. Suites belong to gate and CI, including the ones named in "Done when".
A check that can't run here goes to gate; a real failure still gets fixed.

Reread the docs against what you built. Where behavior is missing, say so
rather than weakening the docs.

## When the plan is wrong

Unclear: make the simplest reversible choice and record it in
`scratch/questions.md`. Contradicted, by a counterexample that breaks the
slice, the data model or the deletion path: stop the work that depends on it
and revise the plan. Never note a contradiction and keep building on it.

Update the plan's remaining work in place as you learn. If you had to discover
an undocumented convention, add it to the agent guide so the next run doesn't
rediscover it.

This is one step of someone's work. Stay in this checkout and build the Task
you were given; don't pick up backlog, create a second Task or start a
competing implementation. Publishing and landing belong to the caller.
