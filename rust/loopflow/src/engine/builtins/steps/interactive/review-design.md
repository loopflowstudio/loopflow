---
interactive: true
requires: scratch/<slug>.md (elaborated design from kickoff)
produces: scratch/<slug>.md (approved or revised) | direction to iterate
default_agent: codex
action_style: exploratory
---
Pressure-test the design with the human before implementation burns time.

Kickoff produced a bold, opinionated design. This step checks whether the boldness points in the right direction. This is the last cheap moment to change it.

## Voice

The human is here to guide the architectural decisions. Open by orienting them in the decision space: what's been decided, what's still open, where their judgment is needed. Not your editorial reaction — their re-entry point into the work.

Don't open by narrowing in on one thing based on interestingness ("The most interesting thing here is...", "What jumps out is...", "The boldest decision..."). Start broad — cover what the design proposes — then let the human decide where to focus.

Vary structure and emphasis based on what this design actually needs. A review that feels the same every time becomes a rubber stamp the human stops reading.

## Opening

Before any evaluation or recommendations, orient the human:

1. **What the design proposes** — the problem, the approach, and the core decisions everything else hinges on.
2. **Key types and APIs** — the data structures and interfaces the design introduces. Quote them from the doc.
3. **What's still open** — decisions that need the human's judgment.

This grounds the conversation. Everything else — alternatives, failure modes, scope — comes after.

## Approach

Use a structure that matches the design and conversation. Don't force a rigid phase protocol.

Pause after each major point. Let the human steer depth and order.

Pick the lenses that matter most here. Combine or skip as needed:

- **Model quality** — are data structures and APIs the clearest expression of product semantics?
- **Scope and seams** — is this the right unit of work? Bias toward keeping architectural chunks whole. Splitting creates backwards-compatibility adapters, dual states, and integration risk that often costs more than a larger change.
- **Alternatives and tradeoffs** — surface real options and sketch them.
- **Failure modes** — identify where this is most likely to break.
- **Execution path** — decide what to fix now vs defer to the wave roadmap.

## Collaborative execution loop

Use review as a working session, not a verdict ceremony.

During the session:
- Fix clear wins directly in the design/code path. If something is obviously better and relatively small, just do it — don't ask permission.
- Co-design unresolved decisions with the user; don't decide alone when tradeoffs are product-defining.
- Near the end, run an explicit scope check. Prefer architectural completeness — a design that lands as one coherent change avoids the cruft of transitional states. Only split when pieces are genuinely independent and each stands on its own.
- If packaging options are genuinely needed, offer 2-3:
  - **Minimal** — smallest safe ship-now set.
  - **One more big push** — one additional high-leverage improvement, then ship.
  - **Do it all** — full intended scope now, with longer cycle/risk.
- For each package, state what lands now, what defers to the wave roadmap, and what extra risk/validation it adds.
- Confirm the user has ingested and validated the updated design with explicit feedback.

End state: updated design is validated by the user and ready to drive implementation/PR progress.

## Quality coverage

By the end of the conversation, the relevant quality dimensions should have been
considered — either addressed or consciously set aside.

If directions are loaded, they define the quality lens. Otherwise, make sure these
areas got appropriate attention:

- User experience (visibility, feedback, consistency)
- Correctness and test confidence
- Reliability, performance, security
- Modularity and change impact

No mandatory format. If a dimension isn't relevant, that's fine — just be sure
it's a conscious choice, not an oversight.

## Guidance

- This is a design review, not a code review. Focus on intent, model, and scope — not implementation details the implementing session will figure out.
- The design doc is for the implementing agent. If something is ambiguous enough that you'd guess wrong, flag it now.
- Don't propose alternatives without sketching them. "Have you considered X?" is useless. "X would look like [sketch] and trade Y for Z" is useful.
- Respect the design's boldness. Kickoff is opinionated by design. Don't sand down every sharp edge — only flag decisions where the risk outweighs the upside.
- If the wave context exists, check the design against wave Goals and Risks. A design that ignores known risks should be called out.

## Wave alignment

If `<lf:wave>` is present:

- Does this design advance the wave's stated Goals?
- Does it respect scope boundaries from the wave README?
- Does it introduce risks the wave already flagged?
- Will the "done when" criteria actually move the wave forward?
