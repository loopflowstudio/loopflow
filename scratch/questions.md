# Open questions and recorded assumptions (LOO-705)

Headless choices, 2026-11-02. Each is reversible; Jack decides.

- **Mechanism chosen: replacement, not a thin stable owner.** The Task listed
  three sketches. A thin versioned owner protocol is old-peer support, which
  Jack rejected (wave memory, LOO-401). Exec-forward of the owner would need
  the old owner to know the new CLI's commands. So: superseded owners end at
  idle, viewers follow through the entry, `lf resume` takes over by build
  identity. Rollover becomes the explicit sweep.
- **Not auto-rolling over at promote.** Promote prints what still runs the
  previous lf. Running rollover inside promote races the owners' own ending
  (within 2s of the receipt settling) and would leave some Sessions resumed
  and others ended. Lazy is uniform: idle Sessions end and come back on
  `lf resume` or when viewed. If Jack wants every idle Session running again
  right after an install, rollover should also resume Sessions that ended
  for supersession (needs a recorded reason on the exit).
- **Build identity for `lf resume` = canonical path, else content digest.**
  Desktop's helper and the CLI artifact are different paths with the same
  bytes; comparing content keeps Desktop and terminal from restarting the
  provider at each other. The owner's own judgement is path-only, since the
  entry's target is content-addressed.
- **Tests make a different build by re-signing a copy (macOS) or appending a
  byte (Linux).** Copies of the test binary are byte-identical, so a takeover
  test needs a differing executable that still runs.
