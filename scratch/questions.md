# Open questions and recorded assumptions (LOO-705)

Headless choices, 2026-11-02. Each is reversible; Jack decides.

- **Gate scope:** automatic viewer following applies to the owner's lifetime
  notification. Explicit rollover calls `end_owner` (SIGTERM) and does not
  send `Moved.successor`; an attached viewer can close while the sweep
  resumes its conversation without a viewer. Record this limit rather than
  adding a second control protocol while LOO-712 owns that boundary. Ordinary
  `lf resume` still opens the moved conversation. The earlier design's claim
  that rollover viewers always follow is too broad.
- **Wave memory corrections proposed by gate:** the LOO-705 paragraph still
  teaches cross-version `Option` parsing and says the Mac/Graph demo is
  unproved. Jack's LOO-712 steer withdraws that parsing claim; the demo record
  now supplies the narrower configured-history and next-step evidence.
  Gate leaves Wave memory read-only; its next authorized curator should use
  `scratch/loo-705-demo.md` and retain that record's limits.

- **Demo scope, 2026-11-02:** use the configured LOO-618 Session for native
  history evidence and a disposable private entry for the released-to-branch
  Graph transition. Do not promote the branch on the main Machine or sweep
  unrelated owners merely to demonstrate it. The explicit headless delegation
  supplies the review decision; it is not a new statement from Jack. Evidence
  and limits: [loo-705-demo.md](loo-705-demo.md).

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
  for supersession (needs a recorded reason on the exit). Jack (2026-11-02):
  LOO-709's 2-minute unviewed grace makes "perhaps always" moot; unviewed
  superseded owners end within minutes on their own.
- **Build identity for `lf resume` = canonical path, else content digest,
  owner against the entry's target.** Desktop's helper and the CLI artifact
  are different paths with the same bytes; comparing content keeps Desktop and
  terminal from restarting the provider at each other while they ship the same
  lf. When Loopflow.app and the installed CLI are different builds, each
  `lf resume` from the other side restarts the provider from history. The
  viewer compares the owner with what the entry launches, not with itself, so
  a development build in the main Home attaches rather than restarting
  (realign, 2026-11-02). The owner, the takeover and rollover's inventory share this
  judgement (`installation::superseded`, implement and realign 2026-11-02);
  viewers follow on the owner's word. So an
  owner on the same bytes as the entry's target never ends for supersession;
  content hashing happens only when the two files differ in path but not in
  length.
- **Viewers following through the entry reverses part of LOO-502** (Jack,
  2026-10-17: "replacement viewers are removed"). The Task's outcome ("`lf
  resume` always lands you in the conversation on the installed lf, with no
  PID killing by hand") asks for it; pinning and probing stay removed.
- **Tests make a different build by re-signing a copy (macOS) or appending a
  byte (Linux).** Copies of the test binary are byte-identical, so a takeover
  test needs a differing executable that still runs.
