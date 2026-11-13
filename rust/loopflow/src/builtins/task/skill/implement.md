---
requires: intended change and an implementation plan
produces: working code, a focused check, and an updated plan
action_style: procedural
---
Turn the design doc into working code.

Working code with rough edges beats perfect code that took too long. A first
draft doesn't satisfy acceptance it hasn't met.

Read `scratch/` (`scratch/<branch>.md` is this PR's design;
`scratch/questions.md` holds open questions) and `AGENTS.md`. Read Wave or PM
context only when the seed names the exact wave, Task or Project; never guess
it or repair access first. A small change may have its plan in the
conversation.

## Keep context within budget

Run `lf context` and bring over-budget scratch under both token and byte limits:
merge duplicates, summarize, drop notes inherited from a stacked parent, and
leave history in git with a reference. Keep live decisions, open work and
contrary findings. Re-run `lf context` after writing. Never raise a limit to
hide overflow. Propose memory changes in scratch; edit `.lf/wave/` only when the
Graph calls for it.

## Workflow

1. **Understand the design.** Find the intended outcome, accepted constraints,
   approach and acceptance checks. Read the affected user docs, including edits
   made since the last pass: they describe the experience to build, so keep
   their corrections rather than regenerating from an older plan. Map the
   concepts, persistence and call paths before choosing where behavior goes.
   Read or add the plan's **Delete — do not maintain** list (files, symbols and
   their exclusive tests and fixtures, plus the behavior and data that must
   survive) and keep it current. Add predecessors you discover.

2. **Implement.**
   - Make the deepest planned deletions first, then build on what remains.
     Remove what this change replaces in the same diff; leave no parallel path.
   - Never repair, refactor or extend a deletion target or its tests. When one
     fails, delete it as planned and keep coverage of the required behavior on
     the surviving path.
   - Move real consumers end to end and include any data migration in the
     deletion cut. Breakage mid-cut is no reason to fix the old path; finish the
     cut, then check. New capabilities need no deletion.
   - Keep one unreleased schema migration per Task and edit it in place. When
     the schema changes again, rewrite it to the final shape; never stack a
     second that alters or drops what the first created. Test the upgrade from
     the last released schema to the finished migration.
   - Functions one at a time, following the signatures. Reshape the existing
     code instead of adding a parallel representation.
   - An indivisible change proceeds in internal slices but ships as one PR.
     Update remaining work as you learn. No flags, v2s, or setup nothing uses
     yet.

3. **Sanity check.**
   - Reread the affected user docs to pick up edits made meanwhile. Check their
     examples against what you built; if behavior is missing, say so rather
     than weakening the docs. Edit them in place.
   - Build the changed code and run the focused test for the changed behavior:
     the narrowest filter that names it, never a whole package or workspace.
     Skip builds for prose-only edits. Don't rerun a passing command without a
     change.
   - One build at a time: never run two compilers, test runners or linters
     against the same build directory at once, in the background or in
     parallel tool calls.
   - Gate starts CI for the suites and selects a few high-risk tests, including
     the design's acceptance checks. Don't run suites early because they appear
     in "Done when".
   - If a check can't run headless here, use a headless equivalent or leave it
     to gate/CI. Ask for a judgment when someone is in the conversation;
     headless, leave it for demo or review. A real build or test failure still
     needs a fix.
   - Update remaining work in place, with one command/result line.

## Rules

**Match existing patterns.** If the codebase uses `@dataclass`, use
`@dataclass`.

**Stay in scope.** Build what the design describes. Scope creep goes in
`scratch/questions.md`, not the code.

**One concept, one representation.** A new type means a new real-world concept.
Legacy/New enums, v2 types, adapters, fallbacks, dual writes, compatibility
shims and parallel stores block by default; use one only when the reviewed
design calls for it and names when it is deleted.

**Write for the load it will carry.** Take the data size and concurrency from
what the repository says it serves (its agent guide or docs), not from the
fixture. For every query you add or change:

- Read its plan (`EXPLAIN` in SQLite and Postgres). A full scan of a table that
  grows with use needs an index on the columns it filters, joins or sorts by.
- Bound what a hot path reads. Code that runs per request, turn or event
  touches its own rows by key, with a `LIMIT` where a result can grow; no
  query in a loop where one query would do.
- Keep writes short. A write transaction or lock holds every other writer
  back; take none when there is nothing to change, and do no slow work inside
  one.
- Check it at realistic size: time it against a store as large as the one it
  will run on, and record the number in the plan.

**Test behavior.** Test what people see, not implementation details. Assert on
results, not mock calls.

## Task context

With a Task, follow its directive, accepted design and Steers. Stay in its
worktree; leave the selected Graph and review steps alone. Don't pick up backlog
work, create a second Task or launch a competing implementation. If a planning
read fails, name it and keep working from what you have; don't repair auth.
Publication, landing and navigation belong to the caller's steps.

With a Wave, notice signals that could help it steer: the outcome, a candidate
measure, what decision it informs, and the cheapest way to produce it. Add the
instrument when it fits this change; otherwise propose it to the Wave. Metrics
are discoveries, not a quota.

If `<lf:wave>` is present, read `.lf/wave/<wave>/GOAL.md` and `MEMORY.md`.
Follow the Wave's intent and recorded decisions. They stay read-only unless the
Graph calls for maintaining them; note drift in `scratch/questions.md`.

## When the design is wrong

If the design is unclear, make the simplest reversible choice and record it in
`scratch/questions.md`.

If implementation turns up a counterexample that breaks the slice, the data
model, the deletion path or the overall design, stop dependent work and revise
the design or send it back to review. Never note a contradiction and keep
building on it.

Keep the plan readable by any later step: what will be built, remaining work,
quotes of what people said about it, one check-result line. Skip what the code or git
already shows. No session instructions or Machine facts; past skill names
without `$`.

## Adaptation

If you had to discover an undocumented convention (error handling, test
structure, naming, imports), add it to `AGENTS.md` so the next Session doesn't
rediscover it.
