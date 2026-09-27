# Installer model reduction — 2026-09-26

Reviewed the full branch from `102522bc2` through `e5d1b106a`, the accepted
newcomer design, and installer owners and consumers. The worktree started
clean. Jack's download → drag → open direction remains the target; this pass
only simplifies the implemented lifecycle prerequisite.

## Model before and after

Before: local promotion, published promotion, and recovery each constructed
`ActiveInstall` and decided which published releases to retain. The completion
repair added recovery of the exact recorded selection after partial archival,
but normal promotion still maintained two separate constructors.

After: all three paths use `active_install_from_switch`. The switch supplies
the target and fallback; the existing active record supplies retained history.
An already recorded target is reused exactly after interrupted archival.
Deleted the two promotion-specific retention blocks and their `ActiveInstall`
construction. No new interface, persisted field, or compatibility adapter.

| Fact | Owner | Consumers |
| --- | --- | --- |
| Retained executable bytes and bundle identity | `ArtifactSet` | Staging, validation, shutdown and recovery |
| Prior/target installation and store | `SwitchReceipt` | Promotion, startup authorization and recovery |
| Installed destinations | `ActivationTargets` | Entry registration, replacement and reopen |
| Selected install and retained release history | `ActiveInstall`, derived through `active_install_from_switch` | Both promotion paths and recovery |
| Window intent and pending completion | `SwitchReceipt.app_was_running` and active receipt presence | Quiesce, finish and cancellation |
| Coordinator executable location | Current process and retained binary staging | Bundled helper re-exec before shutdown |

## Retained boundaries and review

- Keep selection commit separate from completion: ordinary app/helper startup
  must use the target before reopening; archival waits for Home recovery and
  app reopen. Removing either boundary would restore the earlier failure.
- Keep retained paths separate from activation destinations. They represent
  preserved bytes and replaceable files, respectively.
- Keep `app_was_running`: process observation cannot recover window intent
  after shutdown. Do not infer pending work from that historical boolean.
- Keep bootstrap construction: it adopts an existing published install before
  a switch exists. Both promotion paths persist that adoption before deriving
  the next selection. The promotion lock protects the active record read.
- Keep startup's projection separate from completed-install construction:
  startup may need the prior selection while recovery prepares the target.
- Preserve receipt schema, transition validation, promotion flags and retired
  app cleanup. Installed callers and upgrade paths still consume them.
- Swift uses the bundled helper and does not mirror the receipt. Shell
  acquisition still calls promotion. No wire fixture or UI contract changed;
  `release/README.md` remains accurate for this reduction.

## Proof and limits

`cargo test -p loopflow --lib app_activation_tests`: **7 passed**, 1.24 seconds
after compilation. The existing lifecycle fixture now asserts exact retained
release history for first install, published update and development update,
in addition to interruption/retry. `cargo fmt --check`,
`cargo clippy --all-targets -- -D warnings`, and `git diff --check` passed.
No wider suite was rerun; persistence and startup source remain unchanged.

These fixtures use temporary machine roots and native sleep processes; GUI open
is a failing test side effect. They do not prove real promotion, retained-helper
exec/parent-exit survival, Home restart, signed copied-bundle activation, or
clean-account acceptance. Shared PATH setup, destination retention, provider
and GitHub setup, the live UI baseline, and public-channel verification remain
in the full design. No publication, Task completion or Flow navigation occurred.
