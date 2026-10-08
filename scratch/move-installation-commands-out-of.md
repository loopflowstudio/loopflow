# Installation command ownership — LOO-401

Jack Heart's October 7 direction accepts `self` for installation, `config user`,
and removal of screenshot without a replacement. Publication stops at PR review.
Main already renamed Home to Machine and introduced `installation` (#1484).

The implementation selects root `lf open` for opening/focusing the app; no
arguments or deep links. `self` replaces `installation`, including hidden
installation operations and skill export. Doctor retains early recovery dispatch.
Display-name reading lives under `config user`; the unique `user`, `doctor`,
`install` shortcuts retain their existing resolver behavior.
Machine retains only id, observe and ssh. No schema or installation-path change.

## Completed removal

- Deleted screenshot.rs, screenshot_tests.rs, ScreenshotArgs, public dispatch,
  hidden supervisor, and screenshot-only ancestry assertions.
- Removed screenshot/desktop Flow owner mappings, obsolete command variants and
  old owner paths, without aliases or saved-command translation for retired paths.
- Removed browser-capture architecture documentation and named screenshot tool
  instructions. The prohibition on launching a GUI browser for capture remains;
  pr-review uses an available agent capture tool or states the limit.
- Renamed the desktop command module to open; moved user reading out of machine.
- Deleted duplicate config/machine dispatch and the redundant open parser test;
  dispatch routes directly to handlers, with public discovery coverage retained.

Install retains its early startup/recovery boundary, hidden transaction operations,
schedule cadence and shared `install` shorthand used by retained binaries.
Desktop SnapshotService renders its own NSView, and the separate AppleScript
capture scripts do not invoke this CLI feature. Those app features,
published release notes and dated Wave evidence remain intact.

## Acceptance and remaining work

The tree, dispatch, Flow canonicalization, consumers and prompt snapshots now
match the selected ownership. The reference regenerates from Clap. Review found
and removed stale doctor recovery instructions and screenshot-only ancestry
assertions; preflight observation remains covered by its isolated fixture.
No code uses a second path or an old-name alias.
Compression removes the nested command matches and unreachable SSH arm; the CLI
dispatcher selects each handler once. Shared process-group cleanup still serves
provider authentication, Flow execution and read retries, so it remains.

Remaining: publication for PR review; landing is not authorized. Disposable OS-account installation
proofs remain with gate/CI; Desktop app launch is unchanged and was not exercised
in this headless run. The generated reference and prompt snapshots match source.
Public discovery tests exercised the demo: `lf help --all` shows self install/doctor,
config user and root open; machine lists only id/observe/ssh; screenshot and its
supervisor reject. Unknown root words remain available to authored skills, so
removal checks use public lookup as well as Clap beneath valid current owners.
Release's child Wave objective and full memory were reviewed during reconciliation;
its operation-entry lesson is already reflected in these removal checks. LOO-397
owns the broader command map; this Task supplies the implemented owner paths.
No unresolved product decision remains.

Checks: `cargo test -p loopflow --test cli_discovery --test user_cli_tests --test global_commands --test one_machine_tests` — 32 passed, three OS-account cases deferred to isolated gate/CI; `cargo fmt --all -- --check` and `cargo clippy --all-targets -- -D warnings` pass. Earlier owner-tree, recovery, Flow and prompt checks remain applicable; exact commands/results: `4328a5347:scratch/move-installation-commands-out-of.md`.
