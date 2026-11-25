---
requires: a change that already works
produces: what the person felt using it, in their words, and where the work goes next
action_style: conversational
---
Put the change in someone's hands so they can feel it.

This is review by using it, not testing; `qa` already made sure it works. The
question now is whether it feels right: does it do what they hoped, does it
flow, is it the thing they wanted.

## Set the stage

Hand over the real product in the state a person would meet it: built,
running, with realistic data, signed in, at the place to start. For a CLI the
command is ready to type; for an app the window is open. Say in a sentence or
two what changed and where to begin. Don't script their steps or narrate what
they'll see.

If it doesn't run, it isn't ready to feel. Go back to implement rather than
demo something broken.

## Let them feel it

Get out of the way. Answer questions and follow where they go. Reactions tell
you more than verdicts: a pause, a surprise, "huh", "oh nice", the thing they
reach for that isn't there.

When a question helps, make it specific and number it so "2b" is a complete
answer: "does the shortcut or the menu feel more natural?", "was the result
where you expected it?". Never "does this look good?".

A page helps when there's something to compare. Write `scratch/demo.html` with
before and after, or two variants side by side, and the numbered questions:
one self-contained file, light and dark. Say its path.

## Keep their words

Write what they said, word for word with their name, in a note under
`scratch/`, along with what they did that surprised you. Keep their words apart
from your reading of them. "Looks fine" stays "looks fine". Shape reactions
into design changes with them, and save the changes they asked for in the
design.

Then go back to implement with their words, or move on when they're done.
Closing the window, going quiet or the provider exiting is never approval.

Clean up what the demo started: stop the processes you launched, by their own
identity and never by name, and keep anything that holds history.

## Headless

Prepare the built product so someone can open and feel it right away, without
opening focus-stealing windows or leaving background processes behind when your
turn ends. Verify it runs; write `scratch/demo.html` when a comparison page
helps. Stop temporary servers or watchers you started while keeping built
artifacts and history ready to reopen.

Write `scratch/demo.md` with what changed, the exact launch command or app path
ready to run, where to start, numbered questions to judge how it feels and your
recommended next action if nobody intervenes. In your final answer, link
`scratch/demo.md` (and `scratch/demo.html` when written), repeat the launch
command, recommendation and numbered questions so someone can answer from the
conversation. Don't act out a person's reactions or invent feedback.
