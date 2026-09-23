# LOO-292 demonstration — 2026-09-23

## Finish line

The human can refresh main, repeat the command, and create a sibling from current
main through `lf`. A feature invocation refreshes main before integrating it.
The laptop's installed `lf install` converges packages, environment and published
binaries, and its configured job runs the same path. An unchanged/stale success,
fixture-only package execution, or a generated but unloaded plist is insufficient.

## Observed facts

- Reported failure: pinned target
  `bca3f35ac7d4ad24ebed2a30481d0b7ba3a86bde` was not an ancestor of
  `aef8c0c98a975a9bd3ceafc6bcedc3e2da37e408` after `lf rebase` on main.
  This is the human's original observation, not a fresh reproduction here.
- This review started at branch head `a2f9c18cc`, with a clean working tree.
- Canonical main was clean at `0bd9bb90d46a0638c54c042c6b26e94e0b25e8d7`,
  matching the locally known `origin/main`. This does not prove remote freshness.
- Installed `/Users/jack/.local/bin/lf` reports version 0.12.19 and describes
  `install` as an internal transaction, with no public `schedule` subcommand.
- Candidate `target/debug/lf install --help` exposes full refresh and `schedule`.
- No matching refresh plist was found in `~/Library/LaunchAgents`.
  `launchctl print gui/501/com.loopflow.refresh` exits 113: service not found.
- The candidate's schedule implementation invokes installed `lf install`.
  Its CLI delegates schedule generation to canonical main's `scripts/install.py`.
  Canonical main and the installed command do not yet contain this PR.

## Hands-on confirmation pending

`uv run pytest python/tests/test_checkout_refresh.py python/tests/test_install_script.py -q`
passed: **43 tests in 58.71 seconds**. Checkout tests execute the candidate CLI
against local Git remotes, including repeated/advancing upstream, preserved
unpublished commits and staged/working edits, feature rebases, sibling creation,
fetch failure/retry, conflicts, and concurrent sibling callers. Package, release,
and scheduler effects are simulated. This is fixture evidence, not a live
Homebrew install, release promotion, or launchd execution.

The human was asked to run the candidate's absolute path with `rebase` twice
from `/Users/jack/src/loopflow`. Success on an already-current checkout proves
the repeated invocation only; stale-main and advancing-upstream coverage must
remain explicitly labeled as fixtures unless observed on the live checkout.

Next: create a named sibling with the candidate's `lf wt create`, verify its
base against fetched upstream, and run `lf rebase --manual` there. Use the manual
mode to keep the demonstration local. Do not publish merely to demonstrate.

## Remaining deployment proof

The real automatic path is currently absent. A release containing the change
and canonical source containing its schedule command are needed for the intended
installed-path demonstration. Do not install a job pointing at the old command
and claim automatic refresh works. After deployment, run `lf install`, configure
`lf install schedule`, and inspect the loaded job and refresh log; then confirm
repeat installation is convergent and later invocation recovers a missed run.
