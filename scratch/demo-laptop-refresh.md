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
  At initial inspection, canonical main and the installed command did not contain
  this PR. The human's subsequent rebase brought the merged PR onto main.

## Hands-on checkout evidence

`uv run pytest python/tests/test_checkout_refresh.py python/tests/test_install_script.py -q`
passed: **43 tests in 58.71 seconds**. Checkout tests execute the candidate CLI
against local Git remotes, including repeated/advancing upstream, preserved
unpublished commits and staged/working edits, feature rebases, sibling creation,
fetch failure/retry, conflicts, and concurrent sibling callers. Package, release,
and scheduler effects are simulated. This is fixture evidence, not a live
Homebrew install, release promotion, or launchd execution.

The human ran the candidate's absolute path with `rebase` twice from
`/Users/jack/src/loopflow` and supplied the output: the first invocation printed
`Updating main in /Users/jack/src/loopflow...`; both printed
`Main is current; unpublished commits and edits remain local.`
Readback found a clean main with HEAD and origin/main both at
`e0849bad498ff8dfb51b344eba3f7520a44d906e`, advanced from the observed
`0bd9bb90d46a0638c54c042c6b26e94e0b25e8d7`. Its history now includes PR #1273
at `c56340a14`. This proves actual stale-main catch-up and a second successful
invocation on the laptop. Advancing upstream between repeated invocations and
preservation under dirty/divergent conditions remain fixture evidence.

The first human attempt at the sibling commands failed at the shell boundary:
zsh reported permission denied for the `target/debug/` directory, then `cd`
reported that `loopflow.refresh-demo` did not exist. This is not a successful
candidate CLI invocation. The exact cause of the truncated executable path is
unknown; retry instructions use short relative paths without a shell variable.

The human retried and supplied output from `lf rebase --manual` in the new
sibling: `Resetting jack-heart/refresh-demo to main...`, followed by restoration
of scratch. Readback confirmed `/Users/jack/src/loopflow.refresh-demo` exists on
`jack-heart/refresh-demo`, is clean, and HEAD, main, and origin/main all resolve
to `e0849bad498ff8dfb51b344eba3f7520a44d906e`. The real sibling path is now
demonstrated. This disposable branch proves the current-main path; preservation
of authored feature commits and dirty edits remains covered by fixtures.

## Remaining deployment proof

The human ran the candidate's `lf install` from the sibling and supplied live
output. Homebrew downloaded its API inventory and reported all six required
dependencies installed (git, rust, uv, tmux, gh, doppler). `uv` resolved 38 packages
in 3ms and audited 21 in 4ms. The published installer reported v0.12.19 already
installed and exited with `lf 0.12.19`. This proves real package/environment
refresh participation and the current-release skip path. It does not prove a
binary upgrade, recovery after a real package failure, or automatic execution.

Post-install readback still shows the old internal `lf install` interface and
no `com.loopflow.refresh.plist`. Installing the latest published release cannot
activate this merged change until a release includes it.

The real automatic path is currently absent. Canonical source now contains the
schedule command, but installed `lf install --help` still exposes the old internal
transaction interface. An installed release containing the change is needed for
the intended installed-path demonstration. Do not install a job pointing at the old command
and claim automatic refresh works. After deployment, run `lf install`, configure
`lf install schedule`, and inspect the loaded job and refresh log; then confirm
repeat installation is convergent and later invocation recovers a missed run.
