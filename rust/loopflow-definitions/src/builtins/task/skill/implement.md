---
requires: intended change and an implementation plan
produces: one working slice of the plan, a focused check, and an updated plan
action_style: procedural
---
Turn the next slice of the plan into working code.

Your job is two things: choose a slice worth building, then write it well.
Working code with rough edges beats perfect code that took too long.

Read the plan, `scratch/plan.md`, and the agent guide; a small change may have
its plan in the conversation. Read the affected user docs too, including edits
made since the last pass: they describe the experience to build, so keep their
corrections.

## Choose the slice

**Build a substantial end-to-end pass.** Carry the change from what a person
does to what gets stored, then run it. Keep building while the next item is
the same kind of work, and do not start another planned pass when context is
already heavy; stop cleanly with room to close out. Pause when the next item
needs a decision or review, or when what you learned could change the plan.

Size the diff so one person can review it in a sitting. Around 2,000 lines
of non-test change is a soft ceiling. Stop earlier when the work is ready
for review.

**Pick the slice that teaches the most.** Build first what could prove the
plan wrong: the join between two parts, the data shape everything depends on,
the step nobody has tried.

**Let the slice change the plan.** If the pieces don't fit, say so in the plan
and stop building on the part that broke.

A small plan is one slice; inside a Task, every slice goes end to end.

## Build it well

**Own the code you're adding to.** Before writing something, search the whole
codebase for the job, not the location: logging, fetching a file, retrying a
call. The copies are usually far apart, written by another area with the same
need. Then decide. Unify when the job has nuance that each copy gets slightly
wrong, or when it is a large part of the system. Leave a copy alone when the
code is simple and sharing it would tie two areas together for nothing. The
test: can you say where else this job is done, and why you joined it or
didn't?

**Reshape what exists.** Use the constructs the codebase already uses for the
job. No flags, v2 types, adapters, fallbacks, dual writes or setup nothing
uses yet, unless the plan calls for one and says when it is deleted.

**Move real consumers end to end,** with any data migration in the same
slice.

**Write for the load it will carry.** Take data size and concurrency from
what the repository says it serves, not from the fixture. Read the plan of
every query you add or change, bound what a hot path reads, keep writes
short, and time it at realistic size.

**Write the tests.** New behavior gets a test in the same slice, and so does
behavior you changed.

**Finish the slice by deleting.** Remove what the new path replaced, with its
tests and docs. Don't repair or extend code the plan deletes; when its test
fails, delete it and cover the behavior on the surviving path.

**Commit each coherent change** as you finish it, with a message that says
what changed and why. Whatever you leave is committed when the step ends.

## Check

Test the behavior you changed, with the narrowest test that names it, never a
whole package. That is your share. The usual mistake is running too many
tests, not too few. Gate and CI come after you and run the suites, including
the ones named in "Done when", so a full run here only makes everything wait.
Maximize the throughput of the whole software factory, not the certainty of
this step.

One build at a time. Skip builds for prose-only edits, and don't rerun a
passing command without a change. A check that can't run here goes to gate; a
real failure still gets fixed.

Run the slice the way a person would and reread the docs against what you
built. Where behavior is missing, say so rather than weakening the docs.

## When the plan is wrong

Unclear: make the simplest reversible choice and record it in
`scratch/questions.md`. Contradicted, by a counterexample that breaks the
slice, the data model or the deletion path: stop the work that depends on it
and revise the plan. Never note a contradiction and keep building on it.

If you had to discover an undocumented convention, add it to the agent guide
so the next run doesn't rediscover it. When the change is ready for someone to
read, write its reading order as `lf help tour` describes.

## When deciding a loop

- **With `scratch/plan.md`:** return `iterate` while the plan lists a pass not
  yet built, naming that pass in the summary; return `advance` once the planned
  passes are built. Before returning `iterate`, collapse the finished pass in
  `scratch/plan.md` to one line with its commit and note any finding that
  changes the next pass, so `scratch/plan.md` stays lean for downstream steps.
  When a counterexample breaks the plan, note what broke in `scratch/plan.md`
  and return `advance` so review and realign can replan rather than iterating
  onto a broken plan.
- **With no `scratch/plan.md`:** the brief is the plan. Build the outcome in
  one pass and return `advance` (iterating only when a concrete remaining piece
  stays within the brief). If the brief is contradicted or outgrows a bounded
  change, return `blocked` with what broke so the Graph stops before shipping.
