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
- **Benchmark fixture.** `desktop_performance.py run --snapshot`,
  `verify-fixture` and `terminal-feedback --mounted` can keep their owned Session if the
  real `lf` can create and reopen it with a stand-in `claude` in a private
  `LF_HOME`. If that needs seeded Loopflow rows, remove the dependent
  scenarios and `DesktopNativeSessionFixture`. The ordinary `run`, snapshot
  Task opening and headless terminal feedback have independent inputs.
- **`mock-waves` stays.** It is read as a UI state fixture, not part of the
  fake-world workflow.
- **`RepositoryWorkspace.machine` stays** while the benchmark launcher opens
  the app on private homes.
- **Legacy launchd job.** Leave the existing copy's job registered. Document
  manual cleanup in the PR. The packaging check used a temporary bundle so
  it could leave the shared Dev in use by the loo-515 Task.
