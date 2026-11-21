# Open choices (made headless, for review)

- **Dev finds the installed `lf` itself.** A bundle with no helper runs the
  account's public entry (`~/.local/bin/lf`); the launcher passes no
  `LF_BIN`. Passed over: `LF_BIN` from the launching shell's PATH, which on
  this Machine can name a July `~/.cargo/bin/lf` and would replace the live
  server.
- **No successor to the copy.** A branch that changes Rust and Swift together
  sees its Swift against the installed server until the branch lands; a
  private `LF_HOME` shows the new server on an empty Machine. Passed over:
  keeping `run --copy`.
- **One Dev per Machine, last run wins.** `run` quits the open Dev and
  replaces the bundle. Passed over: a bundle per checkout.
- **Benchmark fixture.** `desktop_performance.py run / verify-fixture /
  terminal-feedback` build their one Task and Session by launching the real
  `lf` against a stand-in `claude` in a private `LF_HOME`. If that needs
  seeded rows, the three commands and `DesktopNativeSessionFixture` are
  deleted instead.
- **`mock-waves` stays.** It is read as a UI state fixture, not part of the
  fake-world workflow.
- **`RepositoryWorkspace.machine` stays** while the benchmark launcher opens
  the app on private homes.
- **Leftover launchd job for `~/.lf-copy`** is left registered; another
  Task's Dev (pid 35920, from `loopflow.loo-515-make-task-pr`) is using that
  copy now. Booting it out is a by-hand step named in the PR.
- **Shared Dev bundle.** The live acceptance run will quit the loo-515 Dev
  and replace the bundle its copy's server runs from. Check it is idle
  first.
