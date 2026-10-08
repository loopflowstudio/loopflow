# Installation command ownership — LOO-401

Jack Heart's October 7 direction accepts `self` for installation, `config user`,
and removal of screenshot without a replacement. After reviewing PR #1494 and
the help-warning repair, Jack accepted the change and requested advancing the
Task; the `ship` edge now follows the earlier publication-for-review boundary.
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

## Command discovery demo — 2026-10-07

Demonstrated the design above using this checkout's `target/debug/lf` at HEAD
`d35f71cb3d4619b033e0691423c3f39ff15d732a`. Commands ran from a disposable
ordinary directory with a cleared environment, temporary HOME/LF_HOME and no
provider tools on PATH. This is branch CLI evidence, not installed acceptance.

Selected actual `lf help --all` output:

```text
lf self  Manage the installed Loopflow release and exported skills
lf self doctor  Diagnose installation, storage, Process integrity and scheduled receipts
lf self install  Install the latest published Loopflow release from any directory
lf self install schedule  Install the latest Loopflow at login and weekly by default (macOS launchd)
lf config  Read Loopflow configuration
lf config user  Print the configured participant display name
lf open  Open or focus Loopflow.app
lf machine  Inspect this Machine and observe routes to other Machines
lf machine ssh  Run lf on a Machine or SSH host carrying your local credentials
lf machine id  Print this machine's stable local Machine identity
lf machine observe  Record the current route for a known Machine identity
```

`machine --help` listed only ssh/id/observe. `open --help` showed no positional
arguments. `install --help` and `doctor --help` reported their `self` paths;
`config user --json` and `user --json` both returned the fixture's configured
`"Jack Heart"`. Schedule help retained weekly/daily/hourly/5min.
Public lookup rejected screenshot, __screenshot-supervisor and installation
install with exit 2; self screenshot and machine desktop also rejected beneath
valid owners. Unknown root words can still name authored skills.

Source inspection confirmed [LOOPFLOW.md](../rust/loopflow/src/engine/builtins/LOOPFLOW.md)
retains the GUI-browser capture prohibition and
[pr-review](../rust/loopflow/src/engine/builtins/ops/skill/pr-review.md) uses an
available capture tool or states the limit. Desktop's
[SnapshotService](../swift/LoopflowMac/Services/SnapshotService.swift) renders
its NSView directly. No installation, schedule mutation or app launch ran.

Initial observation, repaired below: machine/self/open `--help` printed
`Process history unavailable: no compatible process ledger for this process`
on stderr. Repeating with an absent LF_HOME reproduced it, with exit 0 and no
runtime directory created. The command-layout outcome passed, but warning-free
help in a fresh directory failed at that revision.

Jack Heart's supplied decisions remain the accepted design. No new human
acceptance was recorded during the initial demo.

### Help warning repair

Jack Heart requested fixing the reported warning before PR review. Process
cleanup warned whenever optional early observation found no compatible ledger.
The diagnostic now belongs to ordinary process admission; optional observation
still records help and parser exits when a compatible ledger exists and retains
debug diagnostics when it cannot. No command-name exception list was added.

Replayed help --all, machine/self/open/install/doctor/user --help and --version
with an absent disposable LF_HOME: every command exited 0 with empty stderr,
and no runtime directory was created. An ordinary config user command against
an incompatible disposable store retained its specific ledger-failure warning
and preserved the database bytes. The initial probe expected the generic warning;
the actual existing diagnostic was `ledger unavailable — Processes are not being
recorded`, and the corrected probe passed without another code change.

Checks: `cargo test -p loopflow --test cli_discovery --test process_ownership_tests --no-run` built; compiled cli_discovery plus process_ownership_tests filters `parser_returns_exact_status_without_admitting_an_early_store` and `early_commands_record_exact_exits_without_initializing_or_migrating` through `scripts/test_network.py` — 22 passed; `cargo fmt --all -- --check` and `cargo clippy --all-targets -- -D warnings` passed.

Review confirmed the warning stays at admission, compatible-ledger history is
unchanged, and successful discovery asserts empty stderr. Recommended next
action at repair completion was publication for review with this evidence.
Installation proofs remain with isolated gate/CI; app launch was not exercised.

### Accepted review and next work

Jack Heart reviewed PR #1494 and said “ok looks good keep advacning the task.”
This accepts the review and requests the next workflow edge, superseding the
earlier stop-before-landing boundary. The Task is at `code`'s `demo` node with
no live Flow; its completed implementation and publication are retained history.
Next: `lf task run LOO-401 ship` runs gate, then `pr land -c`. Include the committed
help repair (`3f29ff57e`) in publication. Gate owns remaining automated proof;
review acceptance does not claim installation or app-launch evidence.
