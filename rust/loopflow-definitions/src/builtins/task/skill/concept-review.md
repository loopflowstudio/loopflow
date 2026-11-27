---
requires: the intended experience, the current design, and a person when a design choice must be made
produces: a page showing whether domain and code concepts line up
action_style: exploratory
---
Step back and decide what the system's concepts and interactions should be, then follow that answer through the data model, APIs and infrastructure.

Start from the experience people want and the constraints they stated. The
current implementation informs the design; it doesn't bind it. Question the
premise even when reviewing a plan: it came from the same pipeline and is not
the standard.

Step back first. Everything in front of you, the request, the Task, the
plan, the diff, the conversation so far, points at an area of the system.
Review that whole area, not the last thing that was said and not only the
lines that changed. Find the part of the system it all touches, widen to the
whole it belongs to, the concepts, the actions and the records behind them,
and review that as it would stand with the work in it. A change can be right
in every line and still be a patch on a concept that is wrong.

Before anything is built, review the whole thing the plan builds and whether
its first slice is the right one. After the change is built, review the area
with the whole change in it against its base, not only the last pass, as
someone seeing it for the first time.

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
is this work, however large the fix, even if it takes another big pass.
Every finding goes on the page, marked as this work or as a separate outcome
stated plainly with what it would take. More behavior is scope, not review.
When a finding turns on a design choice, show it on the page as a
recommendation with what was passed over and why, flagging any choice that
can't be undone once shipped.

Show it. Write `scratch/concept-review.html`, built around one question: do
the concepts people work with and the code's concepts line up?

- the domain concepts: what a person knows about and the actions they take;
- the code concepts: the types, records and APIs behind them;
- each pairing marked yes or no, with what is wrong where it is no: a concept
  the code has and the person never sees, one idea under two names, one name
  for two ideas, an action with no single owner in the code;
- the biggest mismatch called out, and the recommendation explained, before
  and after.

One self-contained file, readable in light and dark, that shows the thing
instead of describing it, marks proposed apart from existing, and ends with
numbered questions so "2b" is a complete answer. When the repository has a
visual language (a design system, the app, its docs, internal tools), borrow
its palette, type and layout. Present it with
`lf desktop files scratch/concept-review.html` and give its path.

`scratch/concept-review.html` is the only file this skill writes; it does not
edit `scratch/plan.md`, code or docs, and it does not decide a loop. With
nobody there, the questions stay on the page; the reply gives its path and
what it found.

## With a person

The page is the review. This is often what stands in for code review: the
work is built, and the question is whether its concepts are right. When a
page already exists for the work exactly as it stands, open with that one
instead of reviewing again. Don't update the plan or the design unless asked;
the person reads the page and decides what happens next.

When they want to go further, explore the biggest mismatch together before
presenting a full redesign, draft the affected usage docs, and regenerate the
page as answers come. Ask about choices that change what the system should
be, never about cosmetic refactors, and keep what they say word for word.
