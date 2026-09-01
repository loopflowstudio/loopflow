---
requires: Runnable behavior and the implementation's Done When claims
produces: attended confirmation | reproducible headless evidence and one exact blocker
action_style: conversational
---
Demonstrate the changed behavior through the real configured path.

On a human-present surface, guide the User through doing and observing the
important behavior themselves. Keep source review secondary to the experience.
Do not claim success or end the interaction until the User explicitly confirms
the demonstration. If it fails, capture the exact gap and leave the work ready
for another implementation pass.

On a headless surface, run the same demonstration autonomously. If one material
product judgment blocks the proof, run `lf ask "<exact request>"`; its human
session stays visible after the session agent is ready, and only the human can
Complete it. Do not manufacture a question merely to
create a checkpoint. If the proof genuinely requires absent-User action beyond
that session, stop
with that exact blocker; the Task's declared human FlowStep owns presentation.

Never treat closing, detaching, provider exit, or lack of response as approval.
