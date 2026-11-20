# Open questions (343ad5)

Choices made headless in `scratch/plan.md`, each reversible. The numbers match
`scratch/concept-review.html`.

1. `loopflow-dev.py run` opens the Machine when the branch's lf opens the store
   and its `server describe --json` equals the installed lf's, and a read-only
   copy at `~/.lf-copy` otherwise. Kickoff had chosen always a copy; the review
   kept today's live path for branches that leave wire and schema alone.
2. The copy is read-only. A writable one would let drags and creates be tried
   on real data and needs the scrub and sandbox `lf clone` had (#1549, deleted
   in #1755).
3. A copy's window says it is a copy only when a change is refused. A standing
   label is left outside this work.
4. Restored windows are fixed in the app (a window names its Machine), not with
   a launcher flag.
5. The word is "copy", not "snapshot".
6. `lf server status` reports `ready` when the server answers; a different
   executable is no longer an error.

The copy is refreshed only by running `run` again.

Settled while writing the docs, each reversible:

7. `lf server status` JSON gains `executable_sha256`, the server's own; the
   plan said only "reports the server's executable hash".
8. The launcher prints "Opening the Machine." on the live path and, with no
   installed lf, opens a copy and says so, as the plan's absent states have it.
9. swift/README.md no longer says "Main-Machine commands forward to installed
   lf; new Sessions and agent children use its stable entry": no code doing it
   was found beyond `delegate_selected` for `lf install server`.
   docs/dev/machines.md still says main-Machine dispatch ignores an inherited
   `LF_BIN`; if that is true of a Dev build's panes, the README sentence comes
   back.
10. docs/troubleshooting.md tells a person who sees the decode Notice to run
    `lf install`. That fits the installed app; in Dev the launcher's choice is
    what avoids it.
11. The README says the socket limit is 103 bytes, the number in the world's
    log line.

Settled while building slice 1, each reversible:

12. The refusal's error code is `invalid`: no client retries it. `unavailable`
    was passed over because clients may retry it.
13. `lf-world copy` copies the Machine its shell is on (`LF_HOME`, else
    `~/.lf`) and that Machine's `config.yaml`.
14. `lf-world copy <dir>` refuses a `<dir>` that exists without `env.json`,
    since it deletes the directory.

Open, exposed by the docs: when the Machine's store has a migration the branch
lacks, `run` names `lf sync main`. In a Task checkout `lf sync main` also undoes
a sync onto another Task. Is that the command to name there, or should it say
"bring this branch up to main" and leave the command to the person?

Not established:

- What launched the installed Loopflow.app with the world's `LF_HOME` on
  November 20 (pid 36218, 01:08:52, `LF_AGENT_CALLER` of
  session_a606ccea3aec49c28186a0e4fe8bb5b8, no `LF_BIN`). One candidate: an
  `lf` command run in a shell that had the world's environment and no
  `LF_DESKTOP`, which opens Desktop by the installed path
  (`rust/loopflow/src/lf/commands/open.rs`, `DesktopApp::select`). Not
  reproduced. If it is that, a world's `env.json` could carry
  `LF_DESKTOP=com.loopflow.mac.dev`.
- Why the Dev app showed "The Machine is away" when its reads failed to
  decode; `ServerLink.lostLink` only names disconnects. The Dev app logged
  nothing about the failure. Slice 4 establishes it before changing the
  sentence.
- Whether `lf task create` on the read-only copy reached Linear before the
  store refused. The error came from SQLite and no Task was filed locally; the
  tracker was not checked.
- Whether a Dev build's lf forwards main-Machine commands to the installed lf.
  swift/README.md says both "runs its bundled CLI on the main Machine's store"
  and "Main-Machine commands forward to installed lf"; only `lf install server`
  was seen to exec the installed lf (`delegate_selected`).
- How tracker and PR rows read in a copy, where refreshes cannot run.
- Whether SwiftUI lets a restored window be dropped before it shows.

Seen and left alone:

- 19 `com.loopflow.server.<digest>` launchd jobs for temporary homes of other
  worktrees' test runs were loaded on November 20, most running
  `target/debug/lf server run`. Nothing removes a custom home's server job.
- `/private/tmp/…/world-backlog/home/server.plist` now points at the installed
  app's lf.
