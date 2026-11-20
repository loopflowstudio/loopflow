# Open questions (343ad5)

Choices made headless in `scratch/plan.md`, each reversible:

- `loopflow-dev.py run` always opens a read-only snapshot at `~/.lf-view`; the
  Dev app stops joining the real Machine. The cost is no live terminals and no
  real changes from a Dev build. Is that loss acceptable, or should a
  wire-identical branch keep a live path?
- The snapshot is refreshed only by running `run` again.

Not established:

- What launched the installed Loopflow.app with the world's `LF_HOME` on
  November 20 (pid 36218, 01:08:52, `LF_AGENT_CALLER` of
  session_a606ccea3aec49c28186a0e4fe8bb5b8, no `LF_BIN`). One candidate: an
  `lf` command run in a shell that had the world's environment and no
  `LF_DESKTOP`, which opens Desktop by the installed path
  (`rust/loopflow/src/lf/commands/open.rs`, `DesktopApp::select`). Not
  reproduced. If it is that, a world's `env.json` could carry
  `LF_DESKTOP=com.loopflow.mac.dev`.
- Why the Dev app showed "The Machine is away" when its snapshots failed to
  decode; `ServerLink.lostLink` only names disconnects. The Dev app logged
  nothing about the failure.
- Whether `lf task create` on the read-only copy reached Linear before the
  store refused. The error came from SQLite and no Task was filed locally; the
  tracker was not checked.

Seen and left alone:

- 19 `com.loopflow.server.<digest>` launchd jobs for temporary homes of other
  worktrees' test runs were loaded on November 20, most running
  `target/debug/lf server run`. Nothing removes a custom home's server job.
- `/private/tmp/…/world-backlog/home/server.plist` now points at the installed
  app's lf.
