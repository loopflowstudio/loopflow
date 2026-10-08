# Installation command ownership — LOO-401

Jack Heart's October 7 direction accepts `self` for installation, `config user`,
and removal of screenshot without a replacement. Publication stops at PR review.
Main already renamed Home to Machine and introduced `installation` (#1484).

Choose root `lf open` for opening/focusing the app; no arguments or deep links.
Rename `installation` to `self`, including hidden installation operations and
skill export. Move doctor there without changing early recovery dispatch. Move
display-name reading to `config user`; the unique `user`, `doctor`, `install`
shortcuts retain their existing resolver behavior.
Machine retains only id, observe and ssh. No schema or installation-path change.

## Delete — do not maintain

- Delete screenshot.rs, screenshot_tests.rs, ScreenshotArgs, public dispatch,
  hidden supervisor, and screenshot-only ancestry assertions.
- Remove screenshot/desktop Flow owner mappings, obsolete command variants and
  old owner paths. Add no alias or saved-command translation for retired paths.
- Remove browser-capture architecture documentation and named screenshot tool
  instructions. Preserve the prohibition on launching a GUI browser for capture;
  pr-review uses an available agent capture tool or states the limit.
- Rename the desktop command module to open; move user reading out of machine.

Preserve install's early startup/recovery boundary, hidden transaction operations,
schedule cadence and shared `install` shorthand used by retained binaries.
Desktop SnapshotService renders its own NSView, and the separate AppleScript
capture scripts do not invoke this CLI feature. Preserve those app features,
published release notes and dated Wave evidence.

## Acceptance and remaining work

The tree, dispatch, Flow canonicalization, consumers and prompt snapshots now
match the selected ownership. The reference regenerates from Clap. Review found
and removed stale doctor recovery instructions and screenshot-only ancestry
assertions; preflight observation remains covered by its isolated fixture.
No code uses a second path or an old-name alias.

Remaining: publish for PR review; do not land. Disposable OS-account installation
proofs remain with gate/CI; Desktop app launch is unchanged and was not exercised
in this headless run. The generated reference and prompt snapshots match source.
Public discovery tests exercised the demo: `lf help --all` shows self install/doctor,
config user and root open; machine lists only id/observe/ssh; screenshot and its
supervisor reject. Unknown root words remain available to authored skills, so
removal checks use public lookup as well as Clap beneath valid current owners.

Checks: `cargo check -p loopflow --all-targets`; `cargo test -p loopflow --test {cli_discovery,user_cli_tests,doctor_tests,global_commands,documented_commands,golden_prompt,one_machine_tests}` (each target named with `--test`); `cargo test -p loopflow --lib {engine::flow_graph::tests,engine::builtins::tests,lf::tests}` (separate filters); `cargo fmt --all -- --check`; `cargo clippy --all-targets -- -D warnings` — pass, 103 focused tests; three OS-account installation cases deferred to isolated gate/CI.
