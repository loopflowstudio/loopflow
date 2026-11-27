# LOO-1117: signing and notary secrets reachable only from main

Finding 2 of the 2026-11-26 security review (`/Users/jack/src/loopflow/scratch/security-review.md:30-36`).

## Outcome

A branch cannot sign. The Developer ID certificate, the notary key and the
Dependabot merge token live in GitHub environments whose branch policy is
`main`; the repository holds no secrets at all. A job on any other ref that
names one of those environments is refused before it reaches a runner.

Nothing changes for `lf install --dev`: `Packages (main)` still signs and
notarizes every green main commit, and the morning notarization check still
runs.

## Design

**Two environments, zero repository secrets.**

| Environment | Branch policy | Secrets | Job |
| --- | --- | --- | --- |
| `signing` | `main` | `APPLE_CERTIFICATE_BASE64`, `APPLE_CERTIFICATE_PASSWORD`, `NOTARY_KEY`, `NOTARY_KEY_ID`, `NOTARY_ISSUER` | `desktop-app` in `main-packages.yml`, `notarytool` in `notarization-check.yml` |
| `dependabot` | `main` | `DEPENDABOT_MERGE_TOKEN` | `enable-auto-merge` in `dependabot-auto.yml` |

Each job declares

```yaml
environment:
  name: signing
  deployment: false
```

`deployment: false` reads the environment's secrets without creating a
deployment record (GitHub workflow syntax, `jobs.<job_id>.environment`: "Set
`deployment` to `false` to use an environment's secrets and variables without
creating a deployment object"). Without it every main commit, every morning
check and every Dependabot PR would add a "deployed to …" entry.

Choices made for review:

- **`DEPENDABOT_MERGE_TOKEN` moves too**, into its own environment. The brief
  names five secrets; the lint it asks for ("any job naming a secret other than
  `GITHUB_TOKEN` declares `environment:`") covers the sixth, and the review
  notes the same branch path reaches this Contents-write token
  (`security-review.md:74`). Passed over: one shared environment, which would
  hand the p12 to the Dependabot job.
- **`workflow_dispatch` on `main-packages.yml` is dropped**, not guarded. With
  it go the `|| github.sha` fallbacks (`:18`, `:30`) and the `event_name`
  branch of the `if` (`:15`). To rebuild an expired artifact, rerun the run
  (`gh run rerun <id>`) or land a commit. Passed over: a `refs/heads/main`
  guard. Either is tidiness; the environment is the protection, since a branch
  can add any trigger to its own copy of a workflow.
- **`notarization-check.yml` keeps `workflow_dispatch`.** `.github/CI.md:223`
  documents running it by hand, and from a branch the environment refuses it.
- **"Every repository secret is referenced by some workflow" becomes "there are
  no repository secrets"**, checked against live settings by a script, because
  a hermetic test cannot list them (`GITHUB_TOKEN` has no secrets scope, and
  `python/tests` mocks the network). The workflows are the manifest: no second
  list of secret names is committed.

## Findings

- **Live settings (read 2026-11-26, names only).** Repository secrets: the five
  signing secrets (set 2026-10-30), `DEPENDABOT_MERGE_TOKEN` (2026-10-04).
  `DOPPLER_TOKEN_PRD` is no longer among them. Environments: `dev` (no branch
  policy, holds `LFD_GITHUB_WEBHOOK_SECRET` from 2026-02-13), `pypi` (empty, no
  policy), `github-pages` (policy `main`, no secrets). No workflow names `dev`
  or `pypi`.
- **Was the token rotated?** Doppler `loopflow`/`prd` lists no service tokens
  (`doppler configs tokens` returns none), so a `prd` config service token is
  already dead. Whether `DOPPLER_TOKEN_PRD` was that kind of token cannot be
  read back from GitHub; Doppler's activity log answers it.
- **Has the hole been used?** `Packages (main)` was dispatched once: run
  41111152406 on 2026-10-30 from
  `jack/loo-688-lf-install-dev-installs-a-signed-loopflowapp-built-from`, the
  branch that built the feature, the day the secrets were set. The only
  dispatch of the notarization check was from `main`. Registered workflows are
  the ten in `.github/workflows/` plus GitHub's Dependabot and Pages ones. Runs
  of since-deleted branch workflows were not enumerated.
- **Every trigger in use resolves to `main`.** The branch policy matches
  `GITHUB_REF` (GitHub, "Deployments and environments"). `workflow_run` and
  `schedule` run on the default branch; `pull_request_target` carries the base
  branch. Dependabot's secret restriction lists `push`, `pull_request`,
  `pull_request_review` and `pull_request_review_comment`, not
  `pull_request_target`, and today's job already reads an Actions secret under
  it (the Dependabot secret list is empty). GitHub's pages do not say outright
  that environment secrets behave the same for a Dependabot
  `pull_request_target` run; the first Dependabot PR after the change shows it.
- **The values come from Doppler.** `loopflow`/`prd` holds all six names, so the
  move is `doppler secrets get NAME --plain | gh secret set NAME --env signing`
  with no value on screen.
- **`lf install --dev` is unaffected.** `main_run`
  (`rust/loopflow/src/lf/commands/install/published.rs:267-288`) lists runs
  with `--branch main --status success`; it never depended on dispatch.
- **`package-build.yml` names no secrets**, so the `packages` job (a `uses:`
  job, which cannot declare an environment) needs none.
- **Public repository on a free plan**: environment secrets and branch policies
  are both available.

## What it does not stop

Environment settings belong to repository admins, and the maintainer's `gh`
token is admin (this planning run read the environments with it). An agent
holding that token can still change the policy through the API. That becomes a
settings change in the audit log, no longer a branch push; closing it is
finding 3's scoped tokens. A merged PR that edits `release-loopflow.py` is
signed as before: that is the product working, bounded by CI and the merge
queue.

## Security and privacy

Protected: the p12 and its password, the notary API key, the Dependabot merge
token. No value is printed, written to a file in the repository, or passed as
an argument; reads of live settings are names only. `lf security-review`
before delivery: yes, on the workflow diff and the check script.

## Surfaces

- `.github/workflows/main-packages.yml`: `environment:` on `desktop-app`; no
  `workflow_dispatch`; revision is `github.event.workflow_run.head_sha`.
- `.github/workflows/notarization-check.yml`: `environment:` on `notarytool`.
- `.github/workflows/dependabot-auto.yml`: `environment:` on
  `enable-auto-merge`. `close-on-red` uses `GITHUB_TOKEN` and stays as it is.
- `scripts/check_actions_secrets.py`: reads the workflows for each
  environment's secret names, reads live settings through `gh api` (names and
  policies), prints every disagreement and exits non-zero: a repository secret
  exists; an environment a job names is missing, lacks a `main`-only branch
  policy, or its secret names differ from what its jobs reference; an
  environment holds secrets no job names.
- `python/tests/test_workflow_secrets.py`:
  - every job in every workflow that mentions `secrets` other than
    `secrets.GITHUB_TOKEN` (including `secrets: inherit` and `toJSON(secrets)`)
    declares `environment:`; no workflow-level `env:` names a secret;
  - `main-packages.yml` has no `workflow_dispatch` and no `github.sha`;
  - the check script's comparison, fed canned settings: clean passes, and each
    disagreement above is reported by name.
- `python/tests/test_release_automation.py:15-37` keeps passing unchanged.
- Docs: `release/SCHEDULE.md:18,34,39`, the comment at
  `.github/workflows/release.yml:13-14`, `.github/CI.md:167,200-202,230`
  (environment secrets, the Doppler-to-environment copy command, the check
  script), `release/README.md:217` if its wording drifts.

## Absent and error states

- A job names an environment that does not exist: GitHub creates it with no
  policy and the job falls back to repository secrets. That is why the
  environments are made before the merge, and why the check script reports a
  missing policy.
- Environment exists, a secret is missing: the certificate import or
  `notarytool` fails as it does today for an empty secret; the notarization
  issue opens.
- `gh` lacks admin when the check script runs: it reports the failed read and
  exits non-zero. A failed read is never "no secrets".

## Order of settings changes

The repository settings steps belong to a person with admin, at ship-review.
The build leaves the exact commands in `.github/CI.md`.

1. Before merge, additive: create `signing` and `dependabot` with a custom
   branch policy of `main`; copy the six values from Doppler into them.
   Repository secrets stay, so main keeps signing either way.
2. Merge.
3. After one green `Packages (main)` and one green notarization check on main:
   delete the six repository secrets and the `dev` and `pypi` environments.
4. Run the acceptance check.

## Acceptance check

```bash
uv run pytest python/tests/test_workflow_secrets.py   # passes
uv run python scripts/check_actions_secrets.py        # prints nothing, exits 0
gh secret list                                        # prints nothing
```

## Demo

```bash
git push origin HEAD:probe/loo-1117
gh workflow run notarization-check.yml --ref probe/loo-1117
gh run watch
```

The run fails with "Branch "probe/loo-1117" is not allowed to deploy to
signing due to environment protection rules" and no step runs. The same
command with `--ref main` goes green.

## Exclusions

- Rotating the certificate or notary key. One branch dispatch is on record and
  it is the feature's own; rotation is a person's call.
- Pinning `uses:` to SHAs, Dependabot `cooldown`, `rust-cache` in release
  builds, the publisher's child environment (finding 4).
- Scoped GitHub tokens for agents and rentals (finding 3).
- Required reviewers on `signing`: they would stop per-commit signing.
- Running the check script on a schedule. No Actions token can list secrets
  without being an admin token in Actions; the cron host could, as a later
  Task.

## What goes

- `workflow_dispatch`, the `github.sha` fallbacks and the `event_name` clause
  in `main-packages.yml`.
- "credential-free" and "never enter … GitHub Actions" wording in
  `release/SCHEDULE.md` and `release.yml` where it is untrue.
- In settings: six repository secrets, environments `dev` and `pypi`,
  `LFD_GITHUB_WEBHOOK_SECRET`.

Must survive: signed, notarized `Loopflow-dmg` on every green main commit; the
daily notarization check and its issue; Dependabot auto-merge and queue entry.

## Passes

One pass: the three workflow edits, the test file, the check script, the docs.

Done when a branch run naming `signing` is refused, main still produces a
notarized DMG, and `gh secret list` is empty.
