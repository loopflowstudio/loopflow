---
requires: none
produces: a sourced answer, or a research note under scratch/, for the question asked
---
Map the territory. Understand what exists before deciding what to change.

Answer the question asked, at the depth asked. A narrow question gets a direct
sourced answer, not an architecture survey; go broader only when the decision
depends on it. Read `scratch/` first and don't re-derive what it already says.
Don't implement, and don't offer critique nobody asked for.

## Where to look

Follow the paths that matter to the question:

1. **Entry**: README, entry points, the happy path. How does someone start?
2. **Architecture**: main modules, data structures, key abstractions.
3. **Data flow**: how information moves through the system.
4. **Dependencies**: what this area depends on, and what depends on it.
5. **Patterns**: the conventions, and where they break.
6. **Tests**: what's tested, and where the gaps are.

## Know what you know

- Keep what you saw apart from what you infer. A source, code path, trace or
  experiment backs a claim; resemblance does not.
- When doubt would change the answer, keep two explanations alive and run the
  smallest safe check that tells them apart.
- Test your model against every source, especially ones that contradict the
  favored explanation. A contradiction means the model is wrong, not that one
  claim needs a patch.
- For parallel research, give each agent a different approach and ask for
  findings and exact gaps, not status. Don't share the favored answer first.

## Hand back

Answer in the conversation when an explanation is enough. For longer research,
update the existing note or write one under `scratch/`: how the pieces fit,
with the tensions, gaps and unused capabilities that bear on the question.
Keep sources attached to claims. Open questions are a valid result; don't force
a recommendation. Write a note for another agent complete in one go; a path
alone carries nothing to another checkout.

When someone is reviewing the answer, show it as a page under `scratch/`: the
answer first, the system as a diagram, sources linked to claims, then
counterexamples and open questions. One self-contained file, readable in light
and dark, ending with numbered questions so "2b" is a complete answer. Say its
path. A one-line answer needs no page.
