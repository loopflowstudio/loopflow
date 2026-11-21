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
- **Benchmark fixture.** Keep the CLI-created Task and Session for snapshot
  opening and reopen checks. Terminal feedback only needs its ANSI file and
  `/bin/cat`, including when mounted.
- **`mock-waves` stays.** It is read as a UI state fixture, not part of the
  fake-world workflow. Keep its loading and unavailable captures. The two
  repository-scoped empty captures still showed loading after the fixture
  repair, so they were removed. Use the existing mock Wave for the sheet
  check and keep the separate live website capture.
- **`RepositoryWorkspace.machine` stays** while the benchmark launcher opens
  the app on private homes.
- **Legacy launchd job.** Leave the existing copy's job registered. Document
  manual cleanup in the PR. The packaging check used a temporary bundle so
  it could leave the shared Dev in use by the loo-515 Task.
- The optional distinct-image check uses the same loading and unavailable
  states as the screenshot manifest. Its former populated launch selected
  loading by default. The empty-workspaces launch smoke remains separate.
