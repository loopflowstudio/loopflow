---
requires: Runnable behavior and the implementation's Done When claims
produces: self-contained scratch notes with demo evidence, human feedback, and the next useful action
action_style: conversational
---
Demonstrate the changed behavior through the real configured path.

On an interactive surface, guide the User through doing and observing the
important behavior themselves. Keep source review secondary to the experience.
Record what the User observed, what they accepted, and what remains wrong or
unclear. Reconsider the design with them when the experience calls for it; save
the revised design and concrete implementation direction before ending review.
A failed demonstration can finish its review with clear feedback for another
implementation pass. Do not describe that completion as successful behavior.

Write the result in a topic-named Markdown note under `scratch/`, or update the
relevant existing note. Make it useful to a reader without this conversation:
name the experience reviewed and its date, observed behavior and human feedback,
agreed design changes, unresolved questions, and the recommended next action
with its proof. Link the current design and supporting evidence. Distinguish
human statements from your interpretation, and agreed changes from proposals.
Keep the current account coherent; mark superseded conclusions and preserve
useful evidence. The note should stand on its own in any later review or work
step, including when this demo runs outside a Flow.

When this is a `human:true` Flow step, follow its Session readiness/completion
protocol. Keep the human feedback, revised artifact references, and remaining
work in the scratch notes; give their exact paths and a short takeaway in the
ready summary, for example `lf session ready "See scratch/search-feedback.md:
implement the agreed empty state; verify recovery after clearing the query"`.
The following loop-decide interprets that evidence and chooses the explicit
edge; this demo supplies no navigation verdict.

On a headless surface, run the same demonstration autonomously. If one material
product judgment blocks the proof, run `lf ask "<exact request>"`; its review
session stays visible after the session agent is ready, and only the user can
Complete it. Do not manufacture a question merely to
create a checkpoint. If the proof genuinely requires absent-User action beyond
that session, stop
with that exact blocker; the declared interactive demo owns presentation.

Never treat closing, detaching, provider exit, or lack of response as approval.
