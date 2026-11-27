---
requires: a change or behavior to assess
produces: a change that works for the people who will use it, with what you saw and what is still wrong
action_style: procedural
---
Use the change the way the people it is for will use it, and fix what breaks.

You are its first user. Nobody should open a demo or merge a PR and find it
broken on the first click; if they would, this step failed.

## Use the real thing

Run the real thing, not its tests: the built app, the installed CLI, the live
page in a browser, the API over the network, the simulator or device. Drive it
the way a person would. Internal setters, test-only endpoints, mocks and
fixtures do not count.

Earlier runs left notes on reaching each surface in `.lf/qa/`, one
file per surface, whose first line says when to read it. List them with
`head -n 1 .lf/qa/*.md` and read the ones for the surfaces this change touches
before you start.

Getting there is your job. Build it, start its services, create accounts and
realistic data, sign in, set up the state a real user would already have. When
no route exists, make one. Hand back only what truly needs a person (their own
credentials, a physical device, a payment, a production approval) and say
exactly what that left unchecked.

Many agents share this Machine: use a debug build of this change (skip
`--release` or full packaging unless performance or packaging is under test),
focus on the journeys this change affects rather than unrelated regression
suites, and stop every server, browser or watcher you started (by its own PID)
when finished.

- Arrive the way a person arrives: a fresh install or first visit, then a
  returning user with data from before this change.
- Do the main job end to end, then what people actually do: wrong input, empty
  and large data, going back, interrupting and resuming, a slow or failed
  network, two things at once.
- Check what it leaves behind as well as what it shows: files, rows, messages,
  processes, and what another surface now reads.
- Look at it. Notice what is confusing, slow or ugly, not only what is wrong.

## Fix and repeat

Repair what breaks within the change's scope, then go through the experience
again from the start, until it works for the people it is for. A change to
what the behavior should be is not yours to make; describe it. Asked for a
read-only review, report instead of fixing.

Run tests only where they tell you something the real path can't reach.

## Keep what you learned

How to launch and drive it belongs in `.lf/qa/<surface>.md`. When you
had to work out a step no note describes, or one a note describes no longer
works, write the working version into that surface's file, replacing what was
wrong. A surface with no file gets one, whose first line says when to read it:
`Read when the change touches …`. Commit the notes with the change.

Show the run in `scratch/qa.page.html` following `lf help page`: each scenario,
what you did, what you saw with screenshots or real transcripts, and what you
fixed. Finish with what works now, what you couldn't reach and why, and what is
still wrong, worst first. Compile with `lf page build scratch/qa.page.html` and
present with `lf page open scratch/qa.html`.
