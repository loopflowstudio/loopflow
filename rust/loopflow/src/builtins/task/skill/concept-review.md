---
requires: the intended experience, the current product design, and a person when a product choice must be made
produces: accepted product decisions and unresolved choices in the working design
action_style: exploratory
---
Step back together: what should the product's concepts and interactions be?
Follow that answer through the data model, APIs, and infrastructure.

## With a person

Start from the experience they want and the constraints they accepted. Treat
implementation as evidence, not a commitment. Walk through one concrete
interaction, including recovery. Draft the affected usage docs and skill
instructions to make the proposed experience tangible before designing code.
Keep already-clear guidance unchanged and unimplemented behavior marked as
proposed.

Show the user's actions, the few concepts representing them, and the APIs that
change their state in one screen. Explore the most consequential mismatch
together before presenting a complete redesign. Ask about choices that change
what the product should be; do not ask someone to adjudicate cosmetic refactors.
Follow corrections and update the working design as decisions land. Keep
proposals distinct from accepted requirements and verified behavior. Use the
person's name when recording decisions.

Write that screen as a page, `scratch/concept-review.html`: actions →
concepts → APIs, before and after, with the one consequential mismatch called
out. The conversation is about what the person sees there; the decisions live
in the working design.

- One self-contained `.html` under `scratch/`: inline CSS and script, no
  network, no build step. Escape code. Readable in light and dark and at narrow
  widths.
- Lead with the decision you need. End with numbered questions, so "2b" is a
  complete answer.
- Show the thing, not a description of it. Mark proposed against existing
  behavior visibly.
- One reading column, generous whitespace, no dashboard chrome or badges. Shape
  the structure to the content; no fixed template.
- Header: Task, step, date, and the files it is built from.
- Say its path. Loopflow Desktop shows `scratch/` pages beside the
  conversation; in a bare terminal, open it in the browser. Regenerate the same
  path when the conversation changes it, and write it last, after the Markdown.

## Follow the experience through

Find distinctions users must learn without benefit, awkward actions, and names
that obscure what happens. Describe the simpler interaction first, then trace
its consequences through types, APIs, ownership, persistence, execution, and
recovery. Look for an entire lifecycle that can become a record, derived state,
or machinery made unnecessary by a clearer product contract.

Challenge the proposal with a normal path, a failure, and recovery. What must
survive? What deliberately changes? What counterexample would invalidate it?
Preserve counterexamples and revise the proposal when it fails. Do not trade
away required behavior or recoverable data for a deletion count. A clearer
interaction is valuable even when the implementation grows.

Use history only to answer a concrete design question. Read the patch and
rationale, including later repairs, before treating a past deletion as precedent.
Keep the full accepted requirements when recording alternatives so a later
agent cannot mistake a proposal for approval.

## Record the decision

Keep a concise account in the existing working design: accepted experience and
model, consequences across layers, unresolved product choices, and next proof.
Do not create a per-slice review or repeat implementation evidence; realign
reconciles the plan and code using what the work has taught us. Do not rewrite implementation or
restart execution unless requested.

## In a Graph without a person

Run the same review against the accepted design and the implemented change.
Settle what accepted product decisions already imply: clearer names, merged
concepts, machinery a clearer contract makes unnecessary. Record those in the
working design as the next implement direction; do not rewrite code here.

A genuine product choice changes what the product should be and is not settled
by an accepted decision. Record each one as unresolved in the working design,
with the exact question and its evidence. Never invent approval for it.

Then report one of three outcomes for the following loop-or-next:
- nothing to change and no open choice: say so briefly; it advances.
- the design changed: name the change; it iterates to implement.
- an open product choice: state the question; it returns Blocked with it.

Do not create a Task, publish, merge, or choose navigation. This step's result
is evidence; loop-or-next decides.
