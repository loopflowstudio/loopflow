---
requires: user intent, current design, core types and APIs, and available review evidence
produces: scratch/concept-review.md with a simpler model or an explicit decision to keep it
action_style: exploratory
---
Simplify the product's concepts and interactions, then follow those improvements through the data model, APIs, and infrastructure.

Use this review to step back with a human mid-task. Start from the experience
they want and the constraints they accepted. Treat the current implementation
as evidence, not a commitment to its design. Making the product easier to
understand and use is worthwhile on its own. Follow that win through every
layer to capture further simplifications; infrastructure deletion is a possible
consequence, not the admission price. A good outcome may clarify one interaction
or remove a whole lifecycle. Keeping an already clear model is valid.

## With a human

Recover the user's intent, then draft the affected usage documentation and skill
instructions to show how people should use the simpler product. Keep clear
guidance unchanged. Walk through one concrete interaction with the human,
including recovery. Use that draft to discover the design before designing or
changing the code. Keep proposals distinct from accepted requirements and
verified behavior. Then show the core model in one screen: the user's actions,
the few types representing them, and the APIs that change their
state. Explore the most consequential mismatch together before presenting a
complete redesign. Ask about product choices that change what should exist;
do not make the human adjudicate cosmetic refactors. Follow their corrections
and update the working design as decisions land. Do not rewrite implementation
or restart execution unless requested.

## Review

1. **Recover intent.** Read the request or Task, current design, relevant diff,
   and prior review findings. State what must become easier and which observable behavior,
   recovery guarantees, and data must survive. Separate accepted constraints
   from assumptions the implementation introduced. Missing intent or review
   evidence is a gap to resolve, not permission to invent acceptance criteria.
2. **Write the usage first.** Rewrite the affected docs and skills around the
   proposed experience: what the user does, what happens, and how they recover.
   Prefer a concrete example to architecture prose. Let this expose design
   choices before specifying implementation. Mark unimplemented behavior as
   proposed; reconcile the docs with verified behavior before shipping. In an
   autonomous pass, draft within the accepted scope and name any product choice
   that needs a human rather than silently adopting it. Keep already-clear usage
   unchanged; a review need not manufacture a rewrite. Preserve the current
   requirements when recording alternatives so a later step cannot mistake an
   attractive proposal for an approved scope change.
3. **Name the concepts.** Map product nouns to types, identities, state, owners,
   and transitions. Trace the public API through one normal path and one failure
   or recovery path. Check vocabulary: does one name describe multiple things,
   or do multiple names describe the same thing? Distinguish a whole workflow,
   a repeated section, one pass, and a unit of work when those differ.
4. **Simplify the experience, then follow through.** Find concepts users must
   distinguish without benefit, awkward actions, or names that obscure what
   happens. Describe the simpler interaction first. Trace its consequences into
   types and APIs, then ownership, persistence, execution, and recovery. Look for
   lifecycles that can become records, state that can be derived, and queues,
   mirrors, flags, or reconciliation that no longer need to exist. Carry the
   simplification through rather than wrapping the old model with a new surface.
5. **Challenge the proposal.** Show the current → proposed experience and
   types/APIs, the behavior preserved or deliberately changed, and any machinery
   made unnecessary. Product clarity is a result even when the code count grows.
   Search remaining callers, writers, readers, and recovery paths. Use the
   smallest check that could disprove the proposal. Preserve counterexamples;
   change the proposal when it fails. Do not trade away required behavior,
   ownership, or recoverable data to make a deletion count look good.

Use history when it can answer a concrete design question. Rank net source
deletions to find candidates, then read the actual patches and product rationale;
exclude generated files, moves, and abandoned features as evidence of a better
model. Check later fixes for capabilities the deletion accidentally lost. Do not
repeat a repository-wide history survey on every automated pass.

For example, giving users one execution lifecycle makes status and recovery
easier to understand; following through can also remove duplicate leases, CRUD,
and recovery machinery. A provider launch represented by a manifest, events, and
terminal receipt makes inspection direct and can make an execution database
unnecessary. Judge the experience and its required behavior first, then capture
the additional gains. No particular architecture or line-count target is required.

## Autonomous review and handoff

After review-slice, reuse its behavioral evidence and inspect the model behind
it. Stay within the selected work and its affected concepts; no tracked Task is
required for this review. Produce a bounded judgment; do not start a speculative
redesign, launch another agent, or wait for a human.
If a product decision is necessary, identify the exact choice and its effects.

Write or update `scratch/concept-review.md`: proposed usage and affected docs/skills,
current model, concrete findings with source evidence, proposed simplification
and preserved behavior, unresolved
decisions, and the smallest next action/proof. Distinguish observed defects from
design alternatives. Keep the current account coherent rather than appending a
transcript. Carry forward unresolved review-slice findings; a clean concept
review does not make failing behavior complete. If a proposed change invalidates
earlier proof, name the affected claims and return them for implementation and
behavior review before completion. A rewritten document is not execution proof.

Return the judgment as review evidence. When the Flow includes loop-decide,
that step owns the navigation choice; a finite Flow or standalone review needs
no added decision step. Name specific remaining work, the next proof, and any
required human choice. A clean model does not
establish that the whole approved work satisfies its Done when claims. Optional
polish or an unselected alternative does not force another pass. Do not record a
navigation decision, invent a command, create a Task to submit the review, or
launch the next step.

This review supplies evidence; it does not choose navigation, publish, merge, or complete the Task.
