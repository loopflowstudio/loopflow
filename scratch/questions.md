# Open choices (made headless, for review)

- **No successor to the copy.** A branch that changes Rust and Swift together
  sees its Swift against the installed server until the branch is installed.
  Passed over: keeping `run --copy`.
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
- **Shared Dev bundle.** `~/Applications/Loopflow Dev.app` is one path for
  every checkout; the live acceptance run will replace the bundle the
  loo-515 Dev is running from. Check it is idle first.
