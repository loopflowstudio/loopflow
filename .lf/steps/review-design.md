---
interactive: true
requires: scratch/<slug>.md (elaborated design from kickoff)
produces: scratch/<slug>.md (approved or revised) | direction to iterate
---
Pressure-test the design with the human before implementation burns time.

Kickoff produced a bold, opinionated design. This step checks whether the boldness points in the right direction. The design doc is a bet — this is the last cheap moment to change it.

## Approach

Use a structure that matches the design and conversation. Don't force a rigid phase protocol.

Pause after each major point. Let the human steer depth and order.

Pick the lenses that matter most here. Combine or skip as needed:

- **Intent and key bet** — summarize the problem, approach, and biggest bet.
- **Scope and seams** — is this the right unit of work, or should it split?
- **Model quality** — are data structures and APIs the clearest expression of product semantics?
- **Alternatives and tradeoffs** — surface real options and sketch them.
- **Failure modes** — identify where this is most likely to break.
- **Execution path** — decide what to fix now vs defer to the wave roadmap.

## Collaborative execution loop

Use review as a working session, not a verdict ceremony.

During the session:
- Fix straightforward issues directly in the design/code path.
- Co-design unresolved decisions with the user; don't decide alone when tradeoffs are product-defining.
- Near the end, run an explicit scope check and offer 2-3 package options:
  - **Minimal** — smallest safe ship-now set.
  - **One more big push** — one additional high-leverage improvement, then ship.
  - **Do it all** — full intended scope now, with longer cycle/risk.
- Use this scope check to balance completeness against reviewability. Too-small changes under-deliver. Too-large changes (past ~2500 LOC) become unreviewable.
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
