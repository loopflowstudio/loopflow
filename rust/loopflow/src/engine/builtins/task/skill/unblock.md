---
requires: blocker, attempted work, evidence, and the human's judgment
produces: agreed direction and a summary for the waiting caller
action_style: exploratory
---
Resolve stalled work with the human so the waiting agent can make a useful next decision.

This skill runs inside the Session opened by Ask. Read the blocker and the
evidence supplied by the caller. Start with what was attempted, what changed,
and why another pass is unlikely to help. Keep observations separate from the
caller's explanation; the explanation may be wrong.

Use the concept-review skill by default when the direction or model needs
reconsideration. Begin with the experience the human wants, rewrite the affected
usage docs and skill guidance, and work through the consequential product choice
together. Follow the agreed simplification through the data model, APIs, and
infrastructure. Keep accepted requirements distinct from proposed alternatives.
If the blocker is already specific—a missing access grant, unavailable evidence,
or one scope choice—resolve that question directly. No redesign is required.

Save agreed changes in the working design and relevant artifacts. Name the next
action and the observation that would show it helped. Preserve any unresolved
constraint; do not manufacture a resolution just to release the caller.

Save the resolved question, human feedback, accepted changes, remaining
uncertainty, and next useful action in the relevant topic note under `scratch/`.
Include enough context and evidence links for a reader outside this Ask; update
the current account without erasing useful observations. Put the exact note
paths and a short takeaway in the completion summary.

When the human has enough to finish, call `lf session ready "<summary>"` with
the decision, changed assumptions or artifacts, remaining uncertainty, and next
action. Readiness leaves the Session open. The human's Complete action returns
the summary to the waiting caller, which reassesses the work. Do not complete
the Session on the human's behalf, choose Flow navigation, or restart the
worker from here. If no Ask Session is bound, give the summary in the current
conversation without inventing one.
