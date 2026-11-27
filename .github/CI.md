# CI

## Scratch, cancellation and macOS path filters

Publish checkpoints with working notes in `scratch/`; hosted CI defers the full
matrix until scratch is clear. `lf pr publish` keeps those notes in place while
`ship-review` is open.

`lf pr submit`, `lf pr arm`, and `lf land` (and `lf pr land`) sync first, clear
branch-added `scratch/` files once against the integration target on the head
they are about to push, and push that clean head once so the full test matrix
starts immediately. Real code conflicts keep `scratch/` on disk for
`sync-conflicts` and clear before pushing once resolved; pure stacked-scratch
conflicts drop automatically. Scratch-free PRs run the full test matrix in
parallel. Implement/compress only build changed code and run its focused test;
CI runs the suites and `lf land --wait-and-fix` (`ci-fix`) repairs failures;
see the verification cadence in [AGENTS.md](../AGENTS.md#when-to-run-what).
Required checks run unattended; defer unavailable checks to capable CI and
leave people's judgment to demo/review. Neither blocks earlier Graph steps.

For checkpoint PRs, `scratch-clear` reports deferred tests and `tests-result`
and the matrix jobs skip. That is not a passing test result, though
`lf pr checks` exits 0 for it: read the lines. The required merge
queue always rejects scratch artifacts and runs the Linux suite before merging.
Missing scratch, classifier failures, and unexpected skipped candidate jobs
still fail the aggregate. Restoring scratch on a later PR head defers its tests.

To trigger full CI on a manual `lf pr publish` without submitting or arming
auto-merge, run `lf scratch clear` first. CI checks the tree with:

```bash
test -d scratch && test -z "$(find scratch -mindepth 1 ! -path 'scratch/.gitkeep' -print)"
```

The repository owns the `scratch-clear` classifier; the native CLI does not exempt
a failing required check by name. Delivery cleanup keeps existing scratch
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
| `macos-process-test` | Rust files containing `target_os = "macos"`, data clones, store copy, OS process identity, Tailscale endpoint discovery, `lf install` |

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
| `swift-test` | macos-26 | app build, complementary headless model/view, transport/CLI and two-Machine groups (six minutes each on PRs), launch-and-quit check, boundary check |
| `macos-process-test` | macos-15, macos-26 | deleted-executable and exact process-authority regression |
| `loopflow-ui-test` | macos-26 | Desktop app/test-runner compile; native iOS model tests on a disposable Simulator |

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

## Dependencies

```bash
grep -rhE 'uses: +[^. ]' .github/workflows | grep -vE '@[0-9a-f]{40}( |$)'   # no output: every action is at a commit
```

Nothing updates a dependency automatically. Libraries are locked by content:
`Cargo.lock`, `uv.lock`, `website/uv.lock`, `swift/Package.resolved` and the
`libghostty-vt` SHA-256 in `rust/loopflow-terminal/build.rs`. Every outside
action in a workflow is pinned to a full commit SHA, with the tag it came from
in a trailing comment, so a tag its owner moves changes nothing here.

A bump is an ordinary Task and PR that someone chose to start:

```bash
cargo update -p <crate>                                   # then scripts/third_party.py refresh
uv lock --upgrade-package <name>
gh api repos/<owner>/<action>/commits/<tag> --jq .sha     # the commit for a new action tag
```

CI on that PR is the check. GitHub's vulnerability alerts stay on as the
prompt; they notify and open no PR.

The compiler and most tools are not pinned: Rust `stable`, the latest uv,
Homebrew's xcodegen and the `ubuntu-latest` image. `rust-toolchain` and
`install-action` are pinned on their `stable` and `nextest` branches, whose
commits default to that tool, so neither needs a `with:` input. The
`install-action` commit also fixes the nextest version (0.9.152) and its
checksum; nextest moves when that pin does.

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
signs and notarizes the DMG with the secrets in the `signing` environment
([Signing secrets](#signing-secrets)), and uploads `Loopflow-dmg`.
`lf install --dev` installs the newest green run with `gh`, app included on a Mac.
The artifacts expire after 14 days; releases stay the durable channel.

The workflow runs only after CI passes on main. To rebuild a run whose
artifacts expired, `gh run rerun <id>`, or land a commit.

### Signing secrets

Push access is not enough to sign as Loopflow. A branch that names the
`signing` environment is refused before any step runs:

```console
$ gh workflow run notarization-check.yml --ref my-branch
$ gh run watch
X Branch "my-branch" is not allowed to deploy to signing due to environment protection rules.
```

A job that names no environment reads every secret as an empty string.

The Developer ID certificate and the notary key (`APPLE_CERTIFICATE_BASE64`,
`APPLE_CERTIFICATE_PASSWORD`, `NOTARY_KEY`, `NOTARY_KEY_ID`, `NOTARY_ISSUER`)
are secrets of the `signing` environment, whose branch policy is `main`. Two
jobs name it: `desktop-app` in `main-packages.yml` and `notarytool` in
`notarization-check.yml`. The repository itself holds no Actions secrets.

```yaml
environment:
  name: signing
  deployment: false   # read the secrets without a deployment record per run
```

Who can sign:

| Holder | Can sign? |
|---|---|
| Push access: a write collaborator, a token with Contents write | No. Any ref but `main` is refused, and main changes only through a merged PR. |
| Repository admin, including an admin `gh` token | Yes, by widening the branch policy first. The next release stops and names the change. |
| The Doppler login for `loopflow`/`prd` | Yes. The values live there. |

`scripts/check_actions_secrets.py` compares the live settings with what the
workflows name. It prints nothing when they agree; otherwise one line per
difference, and it exits non-zero:

```console
$ uv run python scripts/check_actions_secrets.py
repository secret PROBE: any branch can read it; move it to an environment or delete it
environment signing: branch policy allows release/*; expected main only
environment signing: NOTARY_ISSUER is named by notarization-check.yml and is not set
```

It needs a `gh` login that can read repository settings; a read that fails is
reported as a failed read. The daily release runs it in its host preflight, so
a secret put back on the repository or a branch added to the policy stops that
day's release with those lines. `python/tests/test_workflow_secrets.py` fails a
PR whose job reads a secret without declaring an environment.

A new secret for CI goes in an environment with a branch policy, and the job
that reads it names that environment. Never `gh secret set NAME` without
`--env`.

To create the environment or replace a value, as a repository admin. The value
goes from Doppler to GitHub without reaching the terminal:

```bash
gh api -X PUT 'repos/{owner}/{repo}/environments/signing' \
  -F 'deployment_branch_policy[protected_branches]=false' \
  -F 'deployment_branch_policy[custom_branch_policies]=true'
gh api -X POST 'repos/{owner}/{repo}/environments/signing/deployment-branch-policies' \
  -f name=main -f type=branch

for name in APPLE_CERTIFICATE_BASE64 APPLE_CERTIFICATE_PASSWORD \
    NOTARY_KEY NOTARY_KEY_ID NOTARY_ISSUER; do
  doppler secrets get "$name" --plain -p loopflow -c prd |
    gh secret set "$name" --env signing
done

uv run python scripts/check_actions_secrets.py
```

Create the environment before a workflow names it: GitHub creates a missing
environment on first use with no branch policy.

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
`gh workflow run notarization-check.yml`; from any ref but `main` the `signing`
environment refuses it. A red run opens one issue titled
"Notarization is failing" quoting Apple's message, or comments on the open one;
the next green run closes it.

| Apple says | Fix |
|---|---|
| `403. A required agreement is missing or has expired` | The Account Holder accepts the new agreement at developer.apple.com. No code change; retrying fails until then. |
| `401. Unauthenticated` | `NOTARY_KEY_ID`, `NOTARY_ISSUER` or `NOTARY_KEY` is wrong. Fix it in Doppler `loopflow`/`prd` and copy it to the `signing` environment ([Signing secrets](#signing-secrets)). |

Loopflow ships native packages for all four targets. Workspace crates are private
path dependencies; release publication and recovery verify native artifacts,
installer, DMGs, appcasts and website without a crates.io stage.
