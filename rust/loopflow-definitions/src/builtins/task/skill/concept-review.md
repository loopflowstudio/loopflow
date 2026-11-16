---
requires: the intended experience, the current product design, and a person when a product choice must be made
produces: the product design, what people said about it, and open choices in the working design
action_style: exploratory
---
Step back and decide what the product's concepts and interactions should be,
then follow that answer through the data model, APIs and infrastructure.

## With a person

Start from the experience they want and the constraints they've stated. The
current implementation informs the design; it doesn't bind it. Walk through one
concrete interaction, including recovery. Draft the affected usage docs and
skill instructions so the proposed experience is tangible before designing
code. Leave clear guidance alone and mark unbuilt behavior as proposed.

Show, in one screen, the actions people take, the few concepts behind them and
the APIs that change their state. Explore the biggest mismatch together before
presenting a full redesign. Ask about choices that change what the product
should be, never about cosmetic refactors. Update the working design as
answers come, keeping proposals apart from stated requirements and built
behavior. Record what people say word for word, with the speaker's name, never a
verdict like "accepted" or "chose".

Write that screen as `scratch/concept-review.html`: actions → concepts → APIs,
before and after, with the biggest mismatch called out. The conversation is
about the page; the decisions live in the working design.

- One self-contained `.html` in `scratch/`: inline CSS and script, no network,
  escaped code, readable in light, dark and narrow widths.
- Lead with the decision needed. End with numbered questions so "2b" is a
  complete answer.
- Show the thing, not a description of it. Mark proposed against existing
  behavior visibly.
- One reading column, no dashboard chrome or badges, no fixed template.
- Header: Task, step, date, and the files it is built from.
- Say its path. Loopflow Desktop shows `scratch/` pages beside the
  conversation; in a bare terminal, open it in a browser. Regenerate the same
  path as things change, after the Markdown.

## Follow the experience through

Find distinctions people must learn for no benefit, awkward actions, and names
that hide what happens. Describe the simpler interaction first, then trace it
through types, APIs, persistence, execution and recovery. Look for a whole
lifecycle that could become a record, derived state, or nothing at all.

Test the proposal on a normal path, a failure and recovery. What must survive?
What changes on purpose? What counterexample would break it? Keep
counterexamples and revise when the proposal fails. Never trade away required
behavior or recoverable data for a deletion count; a clearer interaction is
worth it even when the code grows.

Use history only to answer a specific design question. Read the patch, its
rationale and later repairs before treating a past deletion as precedent. When
recording alternatives, keep the stated requirements in full so a proposal
can't pass for one of them.

## Record the decision

Keep a short account in the existing working design: the experience and model
it will build, consequences across layers, open product choices, and what to show
next. Write down only what can't be worked out again from the code, git
history or a fresh run: what people said, word for word, what a run showed at
the time, and facts from outside the repo. Quality per word, not
volume. Don't write a per-slice review; realign reconciles plan and code. Don't
rewrite code or restart execution unless asked.

## In a Graph without a person

Run the same review against the working design and the implemented change.
Settle what the design already implies (clearer names, merged concepts,
machinery a clearer design makes unnecessary) and write it into the working
design as the next implement direction. Don't change code here.

A real product choice changes what the product should be and isn't settled by
the design. Write each one into the working design as an open question, with
why it matters. Don't write an answer into the design as settled unless someone
said so; quote what they said.

Then report one outcome for loop-or-next:
- nothing to change and no open choice: say so; it advances.
- the design changed: name the change; it iterates to implement.
- an open product choice: state the question; it returns Blocked with it.

Don't create a Task, publish or merge. Loop-or-next decides.
