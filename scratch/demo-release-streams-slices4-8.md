# Demo: LOO-669 slices 4–8: streams, one linked lf, self-update, Sparkle (2026-10-30)

This was a stand-in demo for Jack Heart, run headless with no one present. The design is in `scratch/release-streams.md` under Build order 4–8 and the two concept-review addenda. Slice 3 was demoed in `scratch/demo-release-streams-slice3.md`.

The branch `lf` was rebuilt at `68255a41ec` (`cargo build --bin lf`).

## Observed live (read-only)

- `lf install --help` lists `--dev | --preview | --stable` and `schedule`. The help also says that with no flag `lf install` follows the stream recorded in `~/.lf/config.yaml` (stable by default).
- `lf install can-open v0.15.0` with a fresh `LF_HOME` printed `v0.15.0 opens this Machine's store` and exited 0. It fetched the live `SHA256SUMS`. v0.15.0 has no `reader-catalog.txt`, so this took the documented skip path.
- `lf release status` with a fresh `LF_HOME` printed `Preview: v0.15.0 (published 2026-10-28)` and `Stable: v0.15.0`. This matches GitHub: v0.15.0 is Latest and newest.
- `bash release/install.sh --channel dev` refused with a pointer: `dev builds come from CI and need gh auth; install a release, then run lf install --dev`.
- `release/README.md` "What we took from other projects" (slice 8) has an adopt/decline table that is ideas only and has an evidence pointer.

## Tests run for this demo

- `pytest test_shell_installer.py test_release_publisher.py test_release_loopflow.py`: 69 passed.
- `swift test --filter AppUpdatesTests`: 4 passed. These cover the following:
  - dev runs no updater
  - the app follows the recorded stream
  - only the installed app updates
  - a new bundle version is adopted once
- Rust install and store suites were already recorded as passing on the combined tree (the Check line in the plan). They were not rerun.

## Not exercised live

- **Real `lf install --preview|--stable|--dev`, `adopt`, or `schedule`.** Install authority resolves the OS account home, not `HOME` (`installation::account_home`). A live run would therefore relink this Machine's `lf` and touch `/Applications/Loopflow.app` and launchd.
- **A notarized Sparkle update.** This needs a published release with an appcast, so it can only be checked after the first release that carries it (v0.16.0). It is already listed as Remaining under slice 7, next to Developer ID signing of dev builds in CI.

## Decision (stand-in)

Accepted. Slices 4–8 behave as the plan describes on every path that can be exercised without changing this Machine's install. No design change.

## Next

Slices 3–8 go out as one PR, #1776 (Jack Heart, 2026-10-29). Its full CI matrix has not run yet: every check except `macos-inputs` and `scratch-clear` was skipped, which suggests the PR is not ready for review. Gate and publish come next. After the first release, check the live Sparkle update and `lf install` adoption.
