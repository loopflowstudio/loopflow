---
requires: the intended experience, the current product design, and a person when a product choice must be made
produces: the product design, what people said about it, and open choices in the working design
action_style: exploratory
---
Step back and decide what the product's concepts and interactions should be, then follow that answer through the data model, APIs and infrastructure.

Start from the experience people want and the constraints they stated. The
current implementation informs the design; it doesn't bind it.

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

Record the result in the working design: the experience and model to build,
what follows from it across layers, and the open product choices. A product
choice changes what the product should be and isn't settled by the design.
Write each as an open question with why it matters; nothing is settled unless
someone said so, quoted. Don't write a separate review, and don't change code.

## With a person

Make it tangible before designing code. Draft the affected usage docs, and
write `scratch/concept-review.html`: the actions people take, the few concepts
behind them and the APIs that change their state, before and after, with the
biggest mismatch called out. One self-contained file, readable in light and
dark, that shows the thing instead of describing it, marks proposed apart from
existing, and ends with numbered questions so "2b" is a complete answer. Say
its path and regenerate it as answers come.

Explore the biggest mismatch together before presenting a full redesign. Ask
about choices that change what the product should be, never about cosmetic
refactors.

## In a Graph, with nobody there

Run the same review against the working design and the implemented change.
Settle what the design already implies (clearer names, merged concepts,
machinery a clearer design makes unnecessary) and write it into the design as
the next direction for implement. Then report one outcome:

- nothing to change and no open choice;
- the design changed, and how;
- an open product choice, with the question.
