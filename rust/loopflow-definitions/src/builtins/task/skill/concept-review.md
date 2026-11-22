---
requires: the intended experience, the current product design, and a person when a product choice must be made
produces: a page showing whether product and code concepts line up; in a Graph, also a plan the next step can build from
action_style: exploratory
---
Step back and decide what the product's concepts and interactions should be, then follow that answer through the data model, APIs and infrastructure.

Start from the experience people want and the constraints they stated. The
current implementation informs the design; it doesn't bind it.

Step back first. Everything in front of you, the request, the Task, the
plan, the diff, the conversation so far, points at an area of the product.
Review that whole area, not the last thing that was said and not only the
lines that changed. Find the part of the product it all touches, widen to the
whole it belongs to, the concepts, the actions and the records behind them,
and review that as it would stand with the work in it. A change can be right
in every line and still be a patch on a concept that is wrong.

Open by naming the area you took and why, so a wrong reading is caught in one
line. A request that names a narrower area is followed.

## The review

Walk one concrete interaction, including its failure and recovery. Find the
distinctions people must learn for no benefit, the awkward actions, the names
that hide what happens. Describe the simpler interaction first, then trace it
through types, APIs, persistence, execution and recovery. Look for a whole
lifecycle that could become a record, derived state, or nothing at all.

Test the proposal on a normal path, a failure and a recovery. What must
survive? What changes on purpose? What counterexample would break it? Keep the
counterexamples and revise when the proposal fails. Never trade required
behavior or recoverable data for a deletion count; a clearer interaction is
worth it even when the code grows.

A finding earns its place by making the design smaller, more direct or more
true to the experience. A model that is wrong in the area this work touches
is this work, however large the fix: take another big pass to get it right.
What this work should do differently goes in the plan, when the review is
updating one. Only a separate outcome goes on the page as outside this work,
stated plainly with what it would take. More behavior is scope, not review.

Show it. Write `scratch/concept-review.html`, built around one question: do
the product's concepts and the code's concepts line up?

- the product concepts: what a person knows about and the actions they take;
- the code concepts: the types, records and APIs behind them;
- each pairing marked yes or no, with what is wrong where it is no: a concept
  the code has and the person never sees, one idea under two names, one name
  for two ideas, an action with no single owner in the code;
- the biggest mismatch called out, and the recommendation explained, before
  and after.

One self-contained file, readable in light and dark, that shows the thing
instead of describing it, marks proposed apart from existing, and ends with
numbered questions so "2b" is a complete answer. Say its path. Don't change
code.

## With a person

The page is the review. This is often what stands in for code review: the
work is built, and the question is whether its concepts are right. When a
page already exists for the work exactly as it stands, open with that one
instead of reviewing again. Change nothing else. Don't update the plan or the design unless asked; the person
reads the page and decides what happens next.

When they want to go further, explore the biggest mismatch together before
presenting a full redesign, draft the affected usage docs, and regenerate the
page as answers come. Ask about choices that change what the product should
be, never about cosmetic refactors, and keep what they say word for word.

## In a Graph, with nobody there

The plan leads straight into more work, so here the review writes its result
into it. Record in `scratch/plan.md` the experience and model to build, what
follows from it across layers, and the recommended way to pursue it, starting
with the slice that would show soonest whether it holds. The plan must stand
without this conversation. The page is the record someone reads afterwards.

Question the premise the way you would with a person in the room; the plan
was written by the same pipeline and is not the standard.

Before anything is built, a redesign costs nothing, so this is where to be
boldest. Review the whole thing the plan is building, not its first slice
and not its details. Then make sure the first slice is the right one for
that whole, and leave the plan ready for implement to succeed: one design
with nothing left over from the one it replaced, and no contradiction
between old and new sections.

After the change is built, review the area with the whole change in it,
against its base and not only the last pass, as someone seeing it for the
first time.
A model that is still wrong then means another full pass through the plan
and the build, not a follow-up.

When the review turns on a product choice, make the bold one and write it into
the plan and the page as a choice made for review: what you chose, what you
passed over and why. The person reviews it as built. Stop for an answer only
when the choice can't be undone once shipped.

Then report one outcome:

- nothing to change;
- the plan changed, and how;
- a choice that can't be undone needs a person, with the question.
