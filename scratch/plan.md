# LOO-1120: Remove Dependabot and stop automatic dependency updates

## Outcome

No bot opens or merges dependency PRs in this repository. `Cargo.lock`,
`uv.lock`, Swift package pins and workflow action versions change only in a PR
a person or their agent wrote and landed through the normal Graph.

The brief: "Dependabot PRs auto-merged on author alone, with no cooldown and no
review, across uv, cargo, swift and github-actions. A compromised upstream
release reached main, the signed dev build and the daily release within a week.
Automatic dependency updates stop; dependencies change by hand."

## State

Built, on `jack-heart/remove-dependabot` (base `472bacbd64`):

- `0586cdf9f4`: `.github/dependabot.yml` and
  `.github/workflows/dependabot-auto.yml` deleted; `.github/CI.md` and
  `scripts/testing.md` no longer describe them.
- `.github/CI.md` short line reflowed (uncommitted until the next commit).

- Pass 1: every outside action pinned to a commit SHA.

## Findings

- **Nothing else in the tree depends on it.** `git grep -i dependabot` hits
  only `release/v0.9.10`, `v0.9.11`, `v0.11.0`, `v0.13.1`, `v0.13.2`
  `NOTES.md`. Those are shipped records and stay. No workflow names
  `DEPENDABOT_MERGE_TOKEN`, `app/dependabot` or the deleted workflow; the
  "keep in sync with ci.yml" coupling died with the file. The deleted workflow
  was the repository's only `pull_request_target` trigger.
- **Deleting the config is enough to stop the PRs.** Dependabot security
  updates can open PRs with no `dependabot.yml`, but here they are off:
  `gh api repos/{owner}/{repo}/automated-security-fixes` returns
  `{"enabled":false}` and `security_and_analysis.dependabot_security_updates`
  is `disabled`. Vulnerability alerts are on (`/vulnerability-alerts` → 204);
  they notify and open nothing, so they stay as the signal for a hand bump.
- **Nothing is in flight.** `gh pr list --author app/dependabot --state open`
  is `[]`.
- **No other updater opens PRs, but build steps update themselves.** No
  Renovate config, no scheduled `cargo update` or `uv lock --upgrade` under
  `.github/` or `scripts/`. Libraries are content-addressed: `Cargo.lock`,
  `uv.lock`, `website/uv.lock`, `swift/Package.resolved` with
  `-disableAutomaticPackageResolution`, `libghostty-vt` by SHA-256 in
  `build.rs`. Actions are not: 39 of the 76 `uses:` lines are four
  individually-owned repositories, and `rust-toolchain` and `rust-cache` run
  in the job that compiles the release `lf`; `setup-uv` runs in the job that
  later imports the Developer ID certificate.
- **The token lives in two places**, not one: the `DEPENDABOT_MERGE_TOKEN`
  Actions secret (`gh secret list`) and a Doppler secret of the same name
  (`doppler secrets --only-names`), plus the GitHub PAT both hold.
- **Five stale branches on origin**: `dependabot/cargo/fancy-regex-0.20.0`,
  `dependabot/cargo/fancy-regex-0.20.1`, `dependabot/cargo/jsonschema-0.61.2`,
  `dependabot/cargo/rusqlite-0.43.0`,
  `dependabot/github_actions/actions/download-artifact-8`.

## Decisions (made for review, not settled)

Source for the second to fifth: `scratch/concept-review.html`, whose questions
nobody has answered. Each takes the review's recommended option.

- **Remove, don't tame.** Passed over: keeping Dependabot with a `cooldown`,
  no auto-merge and required review. That keeps a write-scoped PAT and a
  `pull_request_target` workflow alive to save a `cargo update` now and then,
  and the brief already chose removal.
- **Lock build steps in this PR.** Every outside action is pinned to a
  40-hex commit SHA with the tag in a trailing comment. Without it the
  outcome sentence is false: a moved `v2` reaches the job that compiles the
  release `lf` faster than the one-week path the brief describes, and leaves
  no commit to revert. Passed over: pinning only the four non-GitHub actions
  (two rules to remember); replacing actions with `run:` steps (changes
  caching); landing the removal alone and filing the lock as its own Task;
  narrowing the outcome to libraries only.
- **"By hand" means someone chose to start the bump**, not that someone read
  it. A bump Task with auto on can still merge unread. No new rule; the PR
  body says this so "by hand" is not read as "reviewed". Passed over: bump
  Tasks that always stop for a person, which needs a way to mark such a Task
  that does not exist.
- **The compiler and tools keep floating**: Rust `stable`, latest uv,
  `brew install xcodegen`, `ubuntu-latest` (nextest is fixed by its pin). Passed over: a
  `rust-toolchain.toml`, which makes every Rust release a bump Task.
- **A person revokes the PAT**; an agent may then delete the two secrets and
  the five branches. Whether anything else uses that PAT could not be checked
  from here; if it is shared, keep it and delete only the two
  `DEPENDABOT_MERGE_TOKEN` entries.

## Acceptance check

After merge, on `main`:

```bash
git grep -il dependabot -- ':!release'              # no output
ls .github/dependabot.yml .github/workflows/dependabot-auto.yml   # both: No such file
gh workflow list --all | grep -i dependabot         # no output, or disabled/absent
gh pr list --author app/dependabot --state open     # empty, and still empty a week on
grep -rhE 'uses: +[^. ]' .github | grep -vE '@[0-9a-f]{40}( |$)'   # no output
```

The last line is the check for pass 1: every outside action is at a commit.
Local `uses: ./.github/workflows/…` calls are exempt.

After the outside cleanup:

```bash
gh secret list | grep DEPENDABOT                    # no output
doppler secrets --only-names | grep DEPENDABOT      # no output
git ls-remote --heads origin 'dependabot/*'         # no output
```

## Security and privacy

This change removes attack surface: a PAT with Contents and Pull requests
write, and a `pull_request_target` workflow that merged on author identity.
Nothing new is exposed, so `lf security-review` is not needed for the diff.
The PAT stays valid until it is revoked on GitHub; deleting the two secrets
alone does not revoke it. Revoke first, then delete.

## Exclusions

- Release notes that mention Dependabot: shipped records, unchanged.
- The compiler, uv, nextest, xcodegen and the runner image (decision above).
- A written procedure or cadence for hand bumps. "By hand" means an ordinary
  Task; `third_party/README.md` already covers the notice inventory.
- Turning off Dependabot alerts. They notify and open nothing, and are the
  only remaining prompt for a bump.
- Showing how far behind the lockfiles are. Stale is the chosen trade.

## Deferred findings

From `scratch/concept-review.html`. Separate outcomes, not built here, not
filed. `lf land` clears `scratch/`, so each needs a Task before landing or it
is lost.

- **Release builders resolve without `--locked`.**
  `.github/workflows/package-build.yml:66` runs `cargo build --release` bare.
  One CI job runs `cargo fetch --locked`, so a stale lock fails before merge,
  but the builder itself would resolve fresh versions quietly. Takes: add
  `--locked` on that line.
- **No age floor on a bump.** The brief names "no cooldown"; a hand bump still
  takes whatever was published this morning. Takes: a Task giving bumps a
  minimum release age, starting with uv's `exclude-newer`; a Cargo equivalent
  was not verified.

## Passes

### 1. Lock the build steps (built, this commit)

All 76 `uses:` lines are at 8 commit SHAs. Learned:

- `rust-toolchain@stable` and `install-action@nextest` need no `with:`: the
  branch commits default `toolchain: stable` and `tool: nextest`.
- Pinning `install-action` also fixes nextest at 0.9.152 (its manifest carries
  version and checksum), so nextest no longer floats. The "tools keep
  floating" decision now holds for Rust, uv, xcodegen and the runner image.
- Both `download-artifact` lines are at v8.0.2; `ci.yml` already read v7
  uploads with v8. The signing job's move from v7 to v8 is untested until
  `main-packages.yml` runs on main: v8 fails on a digest mismatch where v7
  warned.
- Not run here: the PR's CI is the test. `release.yml` and
  `main-packages.yml` only call `package-build.yml`, which a PR does not run.

### 2. Review, PR, land

The PR body carries: "by hand" means started by a person, not reviewed by
one; the compiler and tools still float; the two deferred findings, filed as
Tasks first.

### 3. After merge, outside the repository

Irreversible, so a person does these or says to. Order matters:

1. revoke the GitHub PAT behind `DEPENDABOT_MERGE_TOKEN` (a person);
2. `gh secret delete DEPENDABOT_MERGE_TOKEN`;
3. delete `DEPENDABOT_MERGE_TOKEN` from Doppler;
4. delete the five `dependabot/*` branches on origin.

## Done when

Every outside action in `.github/` is at a commit SHA, a week passes with no
Dependabot PR, the token is revoked and absent from both stores, and origin
has no `dependabot/*` branch.
