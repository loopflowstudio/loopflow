# Open choices (LOO-921)

Assumptions taken in kickoff; each is reversible by re-running `install`.

- **Client-side compile mode as the Machine default** (`SCCACHE_CLIENT_SIDE=1`).
  It is the only mode LOO-650 validated for basedirs, and it keeps compiler CPU
  and memory in each worktree's own Cargo process tree. Server mode on the
  pinned revision is untested here.
- **Own socket, shared default cache directory.** The pinned daemon listens on
  `~/.cache/loopflow/sccache-<rev8>.sock`; the cache stays in
  `~/Library/Caches/Mozilla.sccache` under the existing 10 GiB cap. Stock-keyed
  entries age out by LRU. A separate directory would be cleaner and cost up to
  10 GiB more disk until someone deletes the old one.
- **`post-checkout` hook registers new worktrees.** It is the one moment Git
  itself reports a new root. The hooks directory already holds four February
  `lfd` notifier hooks that write to a socket that no longer exists; `install`
  adds its own marked block and leaves those lines alone. Removing them is a
  Machine cleanup nobody has asked for.
- **Local-disk-only build** (`--no-default-features`). If that fails to build at
  the pinned revision, fall back to default features and say so in the PR.
- **Measured at the Machine's `jobs = 2`**, not LOO-650's one job, because that
  is the loop people run. Per-crate hit/miss is then exact only where
  invocations did not overlap; per-command counters stay exact.
- **Three cohorts as the floor.** About 2.5 hours serial on this Machine. Five
  if background load allows.
- **Idle timeout** stays 600 s unless slice 2 shows a root still unloaded at its
  first build. Kickoff observation: the stock daemon had been up 55 minutes
  under shared use, so no 600 s gap occurred in that time.
- **Idle telemetry revision (2026-11-16):** pinned `ServerInfo` exposes basedirs
  and counters, but no last-request timestamp. `status` reports idle seconds as
  null with a reason rather than inventing a countdown. Natural-idle probes
  observe process exit. Cargo install must name package `sccache` because its
  Git repository also contains a test binary package.

- **Cleaner cap observation stays separate.** The installed cleaner is a copied,
  stdlib-only script, run by `/usr/bin/python3`, with neither the repository nor
  its dev dependencies present. Importing build-cache configuration broke ten
  entry-point fixtures (`ModuleNotFoundError: tomlkit`); that attempted change
  was removed. The existing PATH-based cap observation still needs a separate
  installed-entry-point design; cache activation does not change the 10 GiB cap.

- **No defaults from calibration.** The corrected warm comparison is pending.
  Three warm paired cohorts are 2–4 after cohort 1 revealed missing mode-specific
  dependency seeds and an instrumentation basename that bypassed native caching.
  Keep incremental off and the idle timeout unchanged until those rows and the
  production root check are complete.

- **Concept review (2026-11-16): idle rule fired; restart stays unasked.**
  Daemon up 1h38m with 4 of 64 roots unloaded. Taken: sample pinned-daemon gaps
  and set `SCCACHE_IDLE_TIMEOUT` per the design's own rule. Not taken without an
  answer: stopping the daemon when a worktree registers. Also unasked: the
  10 GiB cap, full at review time with 64 roots, against a disk at 98%.
