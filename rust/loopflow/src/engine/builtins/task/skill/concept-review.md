---
requires: a person present, their intended experience, and the current product design
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

Use this skill on request or inside the interactive unblock Session when a
Flow needs a product decision. It is not a routine headless loop step. If no
person is present, state the exact product choice requiring conversation and
return through the caller's existing blocked protocol. Do not invent approval,
create a Task, publish, merge, or choose navigation. Completing the conversation
returns evidence to the caller; loop-decide reassesses it.
