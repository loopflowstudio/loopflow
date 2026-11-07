---
requires: a change that already works
produces: what the person felt using it, in their words, and where the work goes next
action_style: conversational
---
Put the change in someone's hands so they can feel it.

This is review by using it, not testing. `qa` already made sure it works. The
question now is whether it feels right: does it do what they hoped, does it
flow, is it the thing they wanted.

## Set the stage

Hand over the real product in the state a person would meet it: built,
running, with realistic data, signed in, at the place to start. For a CLI, the
command is ready to type; for an app, the window is open. Say in a sentence or
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
before and after screenshots, or two variants side by side, and the numbered
questions. One self-contained file in light and dark; say its path. Loopflow
Desktop shows `scratch/` pages beside the conversation; elsewhere, open it in a
browser.

## Keep their words

Write what they said, word for word with their name, in a topic-named note
under `scratch/`, along with what they did that surprised you. Keep their
words apart from your reading of them. "Looks fine" stays "looks fine". Shape
reactions into design changes together with them, and save the changes they
asked for in the design. Write down only what couldn't be worked out again
later.

Then go back to implement with their words, or move on when they're done. The
caller chooses the next step; leave the Session and Graph in place.

Never treat closing, detaching, provider exit, or lack of response as approval.

## Disposable demos

Before launch, create two private roots outside build output and temporary
directories, which get purged; use any location the repo's cleanup guidance
names. The history root holds the private Machine, provider history and fixture
repository. The runtime root holds the whole copied runtime (the full app bundle
when there is one) and generated output. Fix absolute paths from the start and
never relocate them. Use the history root as `LF_HOME` and the copied CLI's
absolute path as `LF_BIN` for launch, fixture commands and teardown. Record both
roots, the Session, each demo process's exact identity, the teardown command and
how to reopen the private Machine's history in the note. Keep both roots through
rebuilds while those processes live.

When the demo ends, mark each draft and pending request settled or given up. A
person's input is released only by their answer in the conversation. Closing a
viewer, age or a socket failure releases nothing; without an answer both roots
stay. A headless run settles its own input when it ends.

If all input is settled, end only the verified demo processes, confirm they
exited and remove the runtime root. A Session whose draft was edited stays
running even after the draft is sent, and no command stops a Session: `lf resume`
it, exit its provider, and signal only its verified PID if it won't exit. If any
input was given up, leave the processes running and record what is at risk:
saved history can't recreate a dead editor. Never delete the history root. If
teardown is incomplete, keep the runtime root and record what survives and the
next action. No other cleaner, no signaling by process name.

## Headless

On a headless surface nobody can feel it. Say the demo didn't happen and why,
then finish. Don't act out a person's reactions or invent feedback.
