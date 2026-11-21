# 2ac1ee merge verification

PR #2009 merged as `12b66c3e17bceb28701ebc421a6dde8f124329e8`.
`git fetch origin main` moved main to that commit. `git rev-parse HEAD^{tree} origin/main^{tree}`
printed `2d1dcfc6d92137272b39e50b81d01c09ee734de7` twice and
`git diff HEAD origin/main --exit-code` was empty. Built the identical tree
in the existing Task checkout using its build caches. The CLI version stamp
is `e5318d702`. The saved plan, QA transcript and ship review were read from
`fac63506f5:scratch/`.

- `cargo build -p loopflow --bin lf` initially failed in this shallow checkout
  with `apply graph_process_task: no such column: is_local`. With
  `LOOPFLOW_MIGRATION_ORDER=/tmp/2ac1ee-migration-order.txt` (all 39 main
  slugs matched), Cargo finished the dev profile. With inherited `LF_*`
  unset and a fresh `LF_HOME`, `target/debug/lf --version` printed
  `lf 0.16.0+e5318d702 (build e5318d702 on v0.16.0)`.
- `swift build --build-tests --package-path swift -Xswiftc -gnone` linked
  LoopflowMac and the test runner, finished in 71.90 seconds, and reported an
  existing redundant-await warning in DesktopHeadlessTests.
- `scripts/test_desktop.sh --skip-build -Xswiftc -gnone --filter 'WorkCacheTests|WorkspacePanesTests|OwnerStreamTests|SessionsStoreTests|GhosttyTerminalMountTests|WarmPaneMountTests|DesktopHeadlessTests'`
  exited 0. The runner reported 99 tests in 8 suites, with 4 mounted Ghostty
  tests skipped. Dev restored Work without opening a terminal and prepared
  only the selected Task on visit. A restored Dev board tile left its saved
  shell unmounted.
- `ioreg -n Root -d1` reported `"CGSSessionScreenIsLocked"=Yes`. The
  second release Desktop window and mounted pane handoff still need an
  unlocked display. `lf task create` saved linked follow-up
  [de4835](loopflow://loopflow/task/de4835). `lf task sync de4835` reported
  that Linear was rate-limited and the saved change would retry after reset.
- `git tag --contains origin/main` returned no tags. No release contains this
  change yet. The prior QA transcript exercised the older installed 0.16.0
  CLI's prepared argv on a disposable Machine. Holders from before the fix
  adopt the new sizing rules when their Sessions restart.
- The merged wire keeps `Attach.focused` optional. An older relay that
  omits it follows its historical focused-attach behavior. Main's
  `Request::Theme` stores a view's colors and broadcasts them only when it
  owns sizing. Prepared Desktop argv needs no new option. The PR adopted
  the same focus and input ownership rules for all viewers.
- `cargo test -p loopflow-execution --lib transport::` reported 76 passed
  and one failed. The repaint regression passed. The interrupted-paste test
  read the paste end and the child's `reply:next`, then timed out waiting for
  the paste child exit receipt. That assertion predates this merge. A live
  registry records the exit and deletes the receipt, so the assertion can
  miss it. `lf task create` saved linked follow-up
  [062169](loopflow://loopflow/task/062169). Linear was still rate-limited.
- `cargo test -p loopflow --test transport_tests -- an_unfocused_view_leaves_the_session_with_the_view_in_use only_the_sizing_views_terminal_answers_reach_the_agent a_resumed_relay_keeps_focus_across_server_restarts the_latest_view_used_sizes_the_terminal_and_hands_size_back_as_it_leaves --test-threads=1`
  reported 4 passed. A real disposable PTY stayed at 120 columns with an
  unfocused 80-column viewer and received no WINCH. Focus and typing moved
  sizing and theme ownership, and departure restored the most recent live
  view. Background terminal answers stayed out of the agent's input. Real
  CLI relays retained focus over server restart.
- `cargo test -p loopflow --test integration remote_session_open_returns_an_owner_pinned_terminal_command`
  reported 1 passed. Prepared SSH argv stayed owner-pinned and independent
  of initial focus.
- `gh run list --commit 12b66c3e17bceb28701ebc421a6dde8f124329e8 --event push`
  reported [CI success](https://github.com/loopflowstudio/loopflow/actions/runs/44940571507)
  and [macOS on main in progress](https://github.com/loopflowstudio/loopflow/actions/runs/44940571399).
  The push CI skipped the Rust test job. The macOS UI job compiles the
  app for testing and runs iOS tests. The native Mac interaction is in de4835.

**Follow-up.** Advance 2ac1ee. The focused headless checks pass on the main
tree. Follow-ups track the unlocked Desktop check and the older fixture's
teardown race.

`lf task complete 2ac1ee --reason …` printed `2ac1ee: completed`. The
four idle holders left by the disposable CLI fixtures were stopped.
