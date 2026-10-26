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
step, including when this demo runs outside a Graph.

On an interactive surface, the person reviews a page, and the conversation is
about what they see there. Write `scratch/demo.html` as a runbook for what
changed: each action to take, its expected result beside it, and room for what
they saw. For UI, show before and after screenshots. Failures stay on the page.
The page is a view; the Markdown note keeps the record.

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

Discuss the feedback, revised artifact paths and remaining work in the ongoing
conversation. Save their exact paths and a short takeaway with the review evidence.
The caller chooses further work; this demo closes no Session and restarts no Graph.

On a headless surface, run the same demonstration autonomously. If required
product judgment or absent-user action prevents the proof, explain the exact
blocker in ordinary output and stop. The declared interactive demo owns
presentation. Do not manufacture a question to create a checkpoint.

Never treat closing, detaching, provider exit, or lack of response as approval.
