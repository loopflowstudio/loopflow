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
the same kind of work. Pause when the next item needs a decision or review,
or when what you learned could change the plan.

Size the diff so one person can review it in a sitting. Around 2,000 lines
of non-test change is a soft ceiling. Stop earlier when the work is ready
for review.

**Pick the slice that teaches the most.** Build first what could prove the
plan wrong: the join between two parts, the data shape everything depends on,
the step nobody has tried.

**Let the slice change the plan.** If the pieces don't fit, say so in the plan
and stop building on the part that broke.

A small plan is one slice. Work can be divided between Tasks either way, by
layer or end to end; inside a Task, every slice goes end to end.

## Build it well

**Own the code you're adding to.** Before writing something, search the whole
codebase for the job, not the location: logging, fetching a file, retrying a
call. The copies are usually far apart, written by another area with the same
need. Then decide. Unify when the job has nuance that each copy gets slightly
wrong, or when it is a large part of the system. Leave a copy alone when the
code is simple and sharing it would tie two areas together for nothing. The
test: can you say where else this job is done, and why you joined it or
didn't?

**Reshape what exists.** A new type means a new real-world concept. If the
codebase uses `@dataclass`, use `@dataclass`. No flags, v2 types, adapters,
fallbacks, dual writes or setup nothing uses yet, unless the plan calls for
one and says when it is deleted.

**Move real consumers end to end,** with any data migration in the same
slice.

**Write for the load it will carry.** Take data size and concurrency from
what the repository says it serves, not from the fixture. Read the plan of
every query you add or change, bound what a hot path reads, keep writes
short, and time it at realistic size.

**Write the tests.** New behavior gets a test in the same slice, and so does
behavior you changed. Assert on the intended behavior and user experience,
not internal code flow or mechanics.

**Finish the slice by deleting.** Once the new path works, remove what it
replaced, with its tests and docs. Don't repair or extend code the plan
deletes; when its test fails, delete the test and cover the behavior on the
surviving path. A finished slice leaves one way to do its job.

**Stay in scope.** Unifying with code that already does this job is part of
the slice. New behavior is not: other ideas go in `scratch/questions.md`.

**Commit each coherent change** as you finish it, with a message that says
what changed and why. Committing and pushing are always safe here: history
keeps everything, and nothing needs the branch to stay clean. Whatever you
leave is committed for you when the step ends.

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

Mark the plan's remaining work in place: what this slice built, what it
taught, what is next. If you had to discover an undocumented convention, add
it to the agent guide so the next run doesn't rediscover it.

When the change is ready for someone to read, write its reading order as
`lf help tour` describes.

This is one step of someone's work. Stay in this checkout and build the Task
you were given; don't pick up backlog, create a second Task or start a
competing implementation. Publishing and landing belong to the caller.
