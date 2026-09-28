---
description: Put durable lessons where they change future work.
requires: a lesson supported by observed work
produces: updated guidance or enforcement in the lesson's existing owner
action_style: procedural
---
Make what the work taught us useful the next time it matters.

Read the relevant work and evidence. Identify the decision or behavior the
lesson should change, its limits, and whether the existing code or guidance
already captures it. A lesson needs evidence; a plausible explanation alone
is still a hypothesis. If nothing durable was learned, stop without a record.

Update the narrowest existing owner: code or tests for an enforceable invariant,
nearby documentation for a local convention, the skill that exercises a method,
or the repository guide for a rule every task needs. When an identified Wave
owns a durable decision, curate its existing `wave/<name>/MEMORY.md` through the
repository's workflow. Do not create a Wave or a miscellaneous notes file to
store a lesson.

Replace stale guidance and remove duplication. Preserve accepted decisions,
contrary evidence, and limits that would change how the lesson is applied.
Keep only enough rationale or an evidence link to prevent the rule from becoming
folklore. Never put repository-specific policy into a shared customer skill.

Make clear, authorized corrections now and verify changed behavior with focused
proof. No prior gate is required. If the caller supplies a completed gate or a
frozen delivery boundary, identify any proof the edit invalidates and return
material implementation work to that boundary's owner before shipping. Do not
claim an earlier pass covers changed content.

If ownership or the lesson itself remains consequentially ambiguous, state the
exact choice in the existing working context. Finish with what changed, where
it now lives, and any unresolved enforcement gap. Do not write a retrospective
that merely repeats the work.
