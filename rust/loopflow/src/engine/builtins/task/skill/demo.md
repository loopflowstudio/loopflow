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
product judgment blocks the proof and this Work has a parent, run `lf ask` with
that exact question and continue when the Ask settles. Do not manufacture a
question merely to create a checkpoint. When no parent route exists, report the
evidence directly; if the proof genuinely requires absent-User action, request
that exact intervention with `lf ask --user`.

Never treat closing, detaching, provider exit, or lack of response as approval.
