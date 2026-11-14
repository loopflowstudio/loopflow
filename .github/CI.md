# CI

## Scratch, cancellation and macOS path filters

Publish checkpoints with working notes in `scratch/`; hosted CI defers the full
matrix until scratch is clear. Gate turns the checkpoint into a candidate
before it reviews (commit, sync, `lf scratch clear`, `lf pr publish`), so the
matrix runs while it reads the diff. [Gate](../.lf/skills/gate.md) holds the
steps and where the notes go.

`lf pr submit`, `lf land`, and `lf pr land` clear committed scratch additions
too; after gate they find none. Scratch-free PRs, including
small changes and Dependabot updates, run the full test matrix in parallel.
Implement/compress only build changed code and run its focused test. Gate names
the few tests that matter most for the change and reads their result, and the
suites', from CI for the head it pushed; see the verification cadence in
[AGENTS.md](../AGENTS.md#when-to-run-what). The PR body keeps one check-result line.
Required checks run unattended; defer unavailable checks to capable CI and
leave people's judgment to demo/review. Neither blocks earlier Graph steps.

For checkpoint PRs, `scratch-clear` reports deferred tests and `tests-result`
and the matrix jobs skip. That is not a passing test result, though
`lf pr checks` exits 0 for it: read the lines. The required merge
queue always rejects scratch artifacts and runs the Linux suite before merging.
Missing scratch, classifier failures, and unexpected skipped candidate jobs
still fail the aggregate. Restoring scratch on a later PR head defers its tests.

After the final `lf sync`, run `lf scratch clear` and check that `scratch/`
contains only `.gitkeep` before publishing or arming (cached PR copy is consumed
by publication). A sync can restore notes retained during conflict resolution
after an earlier cleanup. Check the final tree with the
same predicate as CI:

```bash
test -d scratch && test -z "$(find scratch -mindepth 1 ! -path 'scratch/.gitkeep' -print)"
```

The repository owns the `scratch-clear` classifier; the native CLI does not exempt
a failing required check by name. Delivery cleanup preserves existing scratch
content, so repositories retaining it must define their own candidate policy.

A new PR update cancels the previous CI run for that PR. Main and merge-group
runs remain independent, except that a merge-queue entry's earlier runs are
cancelled when GitHub rebuilds it (`cancel-superseded`) or when its PR leaves
the queue (`merge-queue-cleanup.yml`).

The macOS jobs (`.github/workflows/macos.yml`) run once per PR, before the
queue, and only where its changes reach them (`scripts/macos_inputs.py`):

| Job | Runs for |
|-----|----------|
| `loopflow-ui-test` | `swift/` (Desktop and iOS), its build scripts, DTO fixtures, server protocol and Read/Write types, Rust `build.rs` and libghostty-vt pin/patches |
| `swift-test` | Swift sources, shared DTO fixtures and server types, Desktop build inputs, the scripts and Python check it runs |
| `macos-process-test` | Rust files containing `target_os = "macos"`, data clones, store copy, OS process identity, `lf install` |

CI changes, the classifier, an unknown diff or a failed classifier run all
three. Ordinary Rust changes reach Swift through the `lf --json` wire, which the
DTO fixtures pin; native build inputs also run both Desktop link paths. The queue
proves the Linux suite on top of main. After merge, `macos-main.yml` runs the jobs the batch since the
`last-green` tag reaches: one run at a time, the waiting run testing the newest
main. Green moves `last-green` to the tested commit. Red leaves `last-green`
where it was, fails the run and names both commits in the run summary:

```
macOS failure on 286889bbec; last-green stays at 5c1eee745d.
```

Nothing reverts automatically. The next push to main tests the batch again from
the same `last-green`, so a flake clears on its own and a real regression stays
red until someone fixes or reverts it. To retest a red run without a new push:
`gh run rerun <id>`.

## Rust and Swift caches

Every job that builds Rust or replays the migration catalog needs a full
checkout (`fetch-depth: 0`) and `origin/main`. This includes lint, architecture,
installation and native app builds. Without merge history, slug migrations
fall back to name order, which can apply a trigger before its columns exist.
When adding one of these jobs, follow `rust-build`'s checkout and history fetch.

CI Rust cache keys include the root workspace's build profiles, which the cache
action's member-manifest discovery omits. Profile changes get fresh dependency
caches; version-only releases retain them. Main publishes shared caches. The
shared Rust build and macOS Rust jobs can also seed missing caches during PR
review; an up-to-date cache is not saved again. Other Rust jobs only save on main.
Cache restoration alone never skips tests. After a merge, main can reuse a
successful merge-group CI run for the identical SHA and workflow, linking that
run in its summary. Missing or unreadable results trigger the full matrix. Main still
restores the shared caches and runs each job whose cache misses; those jobs
refresh their caches and must pass. Candidate PRs and merge groups always execute
every check.

Swift cache keys include the tracked Swift tree, compiler, and SDK. Successful
main jobs save the build and source hashes/timestamps. After checkout, CI restores
an input's cached timestamp only when its contents still match; changed inputs
keep their new timestamps and rebuild. Missing timestamp metadata leaves normal
Swift build detection in control. Source or toolchain changes may require a new
main cache fill; PRs and merge groups only restore shared caches.

## Command timing

CI wraps the expensive Rust, Swift, and Xcode commands with native
`/usr/bin/time`: `-v` on Linux and `-l` on macOS. Their existing job logs retain
elapsed time, user/system CPU seconds, maximum RSS, faults, and context switches
on success or command failure. Swift's failure artifact retains the same output.
Runner cancellation may prevent a final report. Skipped checks have no new measurement.
Compare the same command, source,
runner/toolchain, and cache state; sum user/system time to compare CPU work with
elapsed time. A timing difference alone does not establish contention.

[GNU time](https://www.gnu.org/software/time/manual/html_node/Memory-Resources.html)
reports maximum RSS in KiB; macOS reports bytes. These are the native command
accounting peaks, not simultaneous aggregate process-tree or host physical memory.
Work performed by external services may be excluded, so this does not measure
Docker daemon memory or establish a safe parallel-worker count. Local gate CPU
receipts remain under `.lf/tmp/gate`; use the same native timing command for a
focused local comparison.

## Jobs

See `.github/workflows/ci.yml`. Checks feed the aggregate `tests-result` check.
Rust partitions wait for one shared build; the other suites run independently.

| Job | Runner | Command |
|-----|--------|---------|
| `architecture-check` | ubuntu-latest | map every durable owner, public boundary, provider edge, and named shim; reject stale control vocabulary |
| `scratch-clear` | ubuntu-latest | defer checkpoint PR tests; reject scratch artifacts on queue/main |
| `rust-lint` | ubuntu-latest | `cargo fmt`, `cargo clippy --all-targets -- -D warnings` |
| `rust-build` | ubuntu-latest | compile and archive Rust tests once |
| `rust-test` | ubuntu-latest | five library and three integration partitions; partition zero proves quietness and checkout refresh |
| `migration-check` | ubuntu-latest | verify migration namespaces/history |
| `python-test` | ubuntu-latest | `uv run pytest python/tests/` |
| `website-test` | ubuntu-latest | `cd website && uv run python dev.py test` |
| `e2e-smoke` | ubuntu-latest | both planning-reconnect proofs, then `tests/e2e/test_smoke.sh` |
| `task-installation` | ubuntu-latest | `uv run python scripts/test_task_installation.py` |
| `swift-test` | macos-26 | app build, headless model/view tests, launch-and-quit check, boundary check |
| `macos-process-test` | macos-15, macos-26 | deleted-executable and exact process-authority regression |
| `loopflow-ui-test` | macos-26 | Desktop app/test-runner compile; native iOS model and UI tests on a disposable Simulator |

Candidate and merge-group jobs must all pass for `tests-result` to pass; a
macOS job may skip only when the classifier says it is unreached or outside
pull requests. Both CI
and the local runner collect every Rust test failure with `--no-fail-fast`.
Ordinary integration modules are registered in `rust/loopflow/tests/integration.rs`
and linked once. Add new modules there; Cargo auto-discovery is disabled. Dedicated
installation, release, reconnect, live-provider and process-global fixture targets remain declared in
`rust/loopflow/Cargo.toml`. Nextest runs every case in a separate process.
CI and the local Rust plan run `scripts/check_rust_test_inventory.py` before
compilation and reject unregistered top-level test source files.
Each Rust partition extracts the same revision's library or integration archive into the checkout's
`target/` directory. The reconnect
target is excluded from that archive and executed in the smoke job. No suite
is dropped; the aggregate requires the build and every partition to succeed.
Partitions use six test slots on the ordinary Linux runners. Partition zero
runs the four native quietness cases in separate processes, preserving each
observation window while overlapping independent waits and the separate
checkout-refresh proof. The job waits for both commands and fails if either fails.
The shared Linux build and macOS Rust builds omit debug symbols to reduce
compilation and transfer work.
It retains unoptimized code and debug assertions; CI backtraces have less
source-line detail. Local Cargo profiles are unchanged.
`.github/workflows/architecture-drift.yml`
runs the same architecture command every Monday and retains its JSON report for
90 days; four consecutive runs are the time-based architecture KR evidence.

## Dependabot

Dependabot checks weekly and groups minor/patch updates by ecosystem. Its
auto-merge workflow uses the `DEPENDABOT_MERGE_TOKEN` Actions secret, sourced
from Doppler, with repository Contents and Pull requests write access. Renew
the token before expiry. The built-in `GITHUB_TOKEN` cannot enqueue PRs into
the required merge queue. Successful PR CI explicitly enqueues the unchanged
Dependabot head when it is not already queued; queue CI still gates merging.

```bash
gh pr list --author app/dependabot
gh run list --workflow CI
```

Weekly dependency PRs come from `.github/dependabot.yml` for `uv`, `cargo`, `swift`, and `github-actions`.

`.github/workflows/dependabot-auto.yml` keeps those PRs zero-touch:
- enable squash auto-merge when a Dependabot PR opens or reopens
- when the `CI` workflow fails on a pull-request run, comment and close the matching PR

Keep `workflow_run.workflows: ["CI"]` in sync with `.github/workflows/ci.yml`. Renaming the CI workflow without updating the Dependabot workflow disables the close-on-red path.

## Main packages

`.github/workflows/main-packages.yml` builds the same native `lf` tarballs as the release workflow for every main commit CI passes, from a full-history checkout so `build.rs` sees origin/main's migration order. Each runner extracts its tarball and runs:

```bash
package-smoke/lf --version
package-smoke/lf --help
package-smoke/lf list
```

Both macOS packages build on Apple Silicon runners. The Intel CLI is
cross-compiled for `x86_64-apple-darwin` and smoke-tested under Rosetta.

Its `desktop-app` job builds Loopflow.app around the ARM64 package's exact `lf`,
signs and notarizes the DMG with the `APPLE_CERTIFICATE_BASE64`,
`APPLE_CERTIFICATE_PASSWORD`, `NOTARY_KEY`, `NOTARY_KEY_ID` and `NOTARY_ISSUER`
Actions secrets (copied from Doppler `loopflow`/`prd`), and uploads `Loopflow-dmg`.
`lf install --dev` installs the newest green run with `gh`, app included on a Mac.
The artifacts expire after 14 days; releases stay the durable channel.

### Notarization

When Apple rejects an upload or a wait, the `desktop-app` log prints
notarytool's output above the traceback. The exception carries its first line,
then what is safe next:

```text
Error: HTTP status code: 403. A required agreement is missing or has expired. …
RuntimeError: Notarization upload failed: Error: HTTP status code: 403. A required agreement is missing or has expired …; inspect Apple history before retrying
```

An upload that fails may still have landed, so check `xcrun notarytool history`
before retrying. A failed wait keeps its submission; rerunning resumes it.

`.github/workflows/notarization-check.yml` runs `xcrun notarytool history`
with the same secrets every morning (13:17 UTC), so a broken account shows up
before a package build hits it. Run it by hand with
`gh workflow run notarization-check.yml`. A red run opens one issue titled
"Notarization is failing" quoting Apple's message, or comments on the open one;
the next green run closes it.

| Apple says | Fix |
|---|---|
| `403. A required agreement is missing or has expired` | The Account Holder accepts the new agreement at developer.apple.com. No code change; retrying fails until then. |
| `401. Unauthenticated` | `NOTARY_KEY_ID`, `NOTARY_ISSUER` or `NOTARY_KEY` is wrong. Fix it in Doppler `loopflow`/`prd` and copy it to the Actions secrets. |

Release candidates also run `cargo package --locked -p loopflow`. This compiles
the extracted crates.io archive before tagging, so repository-only embedded
files cannot pass on the strength of native binaries. Publication keeps the
same locked dependencies.
