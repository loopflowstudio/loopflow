# LOO-1117: signing and notary secrets reachable only from main

Finding 2 of the 2026-11-26 security review (`/Users/jack/src/loopflow/scratch/security-review.md:30-36`).

## Outcome

Nothing is signed as Loopflow without a commit on main. The Developer ID
certificate and the notary key live in a GitHub environment whose branch policy
is `main`; a job on any other ref that names it is refused before it reaches a
runner, and a job that names no environment reads empty strings. No secret in
the repository's settings can be read from a branch, and the daily release
reports a setting that says otherwise while it is still in place.

The ways to a signature:

| Way in | Before | After the settings steps | What it leaves |
| --- | --- | --- | --- |
| Push a branch and dispatch `Packages (main)`, or add an `on: push` workflow | Works; the branch and its run can be deleted | Refused, or reads empty strings | |
| Land a PR | Works | Works, as designed: it is how dev builds ship | The commit, its PR and its CI run, for good |
| Widen the branch policy with an admin token (every local agent Session; every rental until LOO-1118), then use a branch | Not needed | Works | A line at the next release, only while still widened |
| Read the p12 from Doppler `prd` on the laptop | Works | Works | Nothing on GitHub |

Push access is still enough to sign, through a PR the holder queues itself:
ruleset 11018096 on `main` (read 2026-11-26) is merge queue, required check
`tests-result`, no required review, no bypass actors, no deletion or
force-push. What the change buys is that the signature costs a commit on main
that passed CI and cannot be removed.

Nothing changes for `lf install --dev`: `Packages (main)` still signs and
notarizes every green main commit, and the morning notarization check still
runs.

## Design

**One environment, `signing`**: branch policy `main`; secrets
`APPLE_CERTIFICATE_BASE64`, `APPLE_CERTIFICATE_PASSWORD`, `NOTARY_KEY`,
`NOTARY_KEY_ID`, `NOTARY_ISSUER`; named by `desktop-app` in
`main-packages.yml` and `notarytool` in `notarization-check.yml`:

```yaml
environment:
  name: signing
  deployment: false
```

`deployment: false` reads the environment's secrets without a deployment
record on every main commit and morning check. Whether the branch policy still
refuses with it set is settled by the probe below, before the merge. If the
probe is not refused, drop the key and accept the records.

**The settings check has a caller.** `scripts/check_actions_secrets.py` runs in
the release preflight (`scripts/publish_release.py`, beside `gh auth status`).
A repository secret put back, or a branch added to the policy, stops that day's
release with a message naming the difference.

Choices made for review (headless; none is settled, each can be undone):

- **The promise names a place, not a holder**: "Nothing is signed as Loopflow
  without a commit on main", with the ways in and what each leaves, in this
  plan and `.github/CI.md` (written). Passed over: "Push access is not enough
  to sign" with a holder table, which the ruleset on main makes false; the
  same table with its first row corrected, whose opening sentence stays false
  and whose rows change with LOO-1118.
- **The check starts from GitHub's list of environments**, not from the names
  in workflow files. One rule: an environment that holds a secret allows
  `main` only. Passed over: reading only the environments jobs name, which
  never looks at `dev` (open, one secret, readable from a branch today).
- **`dev` gets a `main` branch policy in the settings steps.** It can be
  undone, it closes the read from a branch, and it keeps the daily release
  from stopping on `dev` while its deletion is unanswered. Passed over:
  deleting it, which discards a value nobody can read back.
- **"Set and no job names it" is dropped from the check.** A secret no job
  reads, in an environment only main can use, is not readable from a branch,
  and with every environment read the line would stop the release on `dev`'s
  webhook secret for good. Passed over: keeping it for environments a job
  names, which keys the settings by the files again.
- **`deployment: false` stays, and no use of `signing` is recorded beyond the
  run.** The docs say the check sees a change still in place. Passed over:
  dropping the key so every use shows under Deployments with its branch (an
  entry per main commit and morning check; an admin can delete entries).
- **The daily release stops when the settings are wrong**, as built. Passed
  over: releasing with a warning nobody is made to read.
- **No `dependabot` environment.** This Task covers the five signing secrets;
  LOO-1120 deletes the workflow and the sixth secret. Passed over: its own
  environment, which costs a workflow edit that conflicts with a deletion and
  rests on a GitHub behaviour nobody has confirmed (environment secrets under a
  Dependabot `pull_request_target`). If #2054 stalls, the merge token stays a
  repository secret a branch can read until it lands.
- **This builds on LOO-1120.** The sync brings in the Dependabot workflow
  removal and pinned actions. The lint needs no Dependabot exemption.
  Keep the action pins when editing `main-packages.yml`.
- **The release preflight runs the settings check.** Passed over: no script,
  commands in `.github/CI.md` only; a script a person runs when they think of
  it. Depends on the cron host's `gh` token being able to list environment
  secrets and policies: check that first in the build
  (`gh api repos/{owner}/{repo}/environments/signing/secrets` on the host, once
  the environment exists). If it cannot, leave the call out of the preflight
  and record that in `scratch/questions.md`; a failed read must not stop every
  release.
- **`workflow_dispatch` on `main-packages.yml` is dropped**, not guarded. With
  it go the `|| github.sha` fallbacks and the `event_name` branch of the `if`.
  To rebuild an expired artifact, `gh run rerun <id>` or land a commit. A guard
  in the file protects nothing a branch cannot delete.
- **`notarization-check.yml` keeps `workflow_dispatch`.** `.github/CI.md`
  documents running it by hand, and from a branch the environment refuses it.
- **"Every repository secret is referenced by some workflow" becomes "there are
  no repository secrets"**, checked against live settings by the script,
  because a hermetic test cannot list them (`GITHUB_TOKEN` has no secrets
  scope, and `python/tests` mocks the network). The workflows are the manifest:
  no second list of secret names is committed. Until LOO-1120's secret deletion
  is done the script reports `DEPENDABOT_MERGE_TOKEN`, truthfully.

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
  `schedule` run on the default branch.
- **The values come from Doppler.** `loopflow`/`prd` holds the five names, so the
  move is `doppler secrets get NAME --plain | gh secret set NAME --env signing`
  with no value on screen.
- **`lf install --dev` is unaffected.** `main_run`
  (`rust/loopflow/src/lf/commands/install/published.rs:267-288`) lists runs
  with `--branch main --status success`; it never depended on dispatch.
- **`package-build.yml` names no secrets**, so the `packages` job (a `uses:`
  job, which cannot declare an environment) needs none.
- **Public repository on a free plan**: environment secrets and branch policies
  are both available.

- **Dependabot is leaving.** PR #2054 (LOO-1120, branch
  `jack-heart/remove-dependabot`, open on 2026-11-26) deletes
  `dependabot-auto.yml`; its plan revokes the merge token and deletes the
  secret from GitHub and Doppler. Its `main-packages.yml` pins include
  `download-artifact` v8.0.2, which fails on digest mismatches that v7
  warned about. The first main packaging run exercises that change.
- **The release preflight** is `scripts/publish_release.py:161-166`
  (`security find-identity`, `gh auth status`).
- **`deployment: false` and branch policies.** GitHub's page says reviewers and
  wait timers still apply with it and names "restrict which branches can use
  certain secrets via environment branch policies, without deployment
  tracking" as a use; its table has no branch-policy row. Not yet seen on a
  real run.
## Docs written first

Intended behavior, written before the build: `.github/CI.md` (Main packages,
new "Signing secrets" section, Notarization), `release/SCHEDULE.md`,
`release/README.md` (Automation rhythm). The build makes the script's output
and the refusal message match them, or corrects the docs to what a run shows.
`release.yml:13-14` is left as it is: that workflow names no secrets.

## What it does not stop

The table's last three rows. Anyone who can land a PR can sign what it
contains. An agent holding the admin token can still change the policy through
the API; the preflight notices at the next release if the change is still in
place, not before a signing run. Any local process can read the p12 from Doppler `prd`. A
merged PR that edits `release-loopflow.py` is signed as before: that is the
product working, bounded by CI and the merge queue.

## Security and privacy

Protected: the p12 and its password, the notary API key. No value is printed,
written to a file in the repository, or passed as an argument; reads of live
settings are names only. `lf security-review` before delivery: yes, on the
workflow diff and the check script.

## Surfaces

- `.github/workflows/main-packages.yml`: `environment:` on `desktop-app`; no
  `workflow_dispatch`; revision is `github.event.workflow_run.head_sha`.
- `.github/workflows/notarization-check.yml`: `environment:` on `notarytool`.
- `scripts/check_actions_secrets.py`: reads the workflows for each
  environment's secret names, reads live settings through `gh api` (names and
  policies), prints every disagreement and exits non-zero: a repository secret
  exists; any environment holding a secret lacks a `main`-only branch policy;
  an environment a job names is missing, or lacks a name the job reads. A
  failed read is reported as a failed read, never as "no secrets".
- `scripts/publish_release.py`: the preflight calls the check.
- `python/tests/test_workflow_secrets.py`:
  - every job in every workflow that mentions `secrets` other than
    `secrets.GITHUB_TOKEN` (including `secrets: inherit` and `toJSON(secrets)`)
    declares `environment:`; no workflow-level `env:` names a secret;
  - `main-packages.yml` has no `workflow_dispatch` and no `github.sha`;
  - the check script's comparison, fed canned settings: clean passes, and each
    disagreement above is reported by name.
  The lint reads files a branch can rewrite; it catches an honest slip before
  merge. The settings are the protection and the script is what reads them.
- `python/tests/test_release_automation.py:15-37` keeps passing unchanged.
- Docs: `release/SCHEDULE.md:18,34,39`, the comment at
  `.github/workflows/release.yml:13-14`, `.github/CI.md:167,200-202,230`
  (the ways-in table, environment secrets, the Doppler-to-environment copy
  command, the check script and its preflight caller), `release/README.md:217`
  if its wording drifts.

## Absent and error states

- A job names an environment that does not exist: GitHub creates it with no
  policy and the job falls back to repository secrets. That is why the
  environment is made before the merge, and why the check script reports a
  missing policy.
- Environment exists, a secret is missing: the certificate import or
  `notarytool` fails as it does today for an empty secret; the notarization
  issue opens.
- Between the merge and the deletion of the repository secrets, the release
  preflight reports them and stops a release that falls in the gap. Do the
  settings steps the same day.

## Order of settings changes

The repository settings steps belong to a person with admin, at ship-review.
The build leaves the exact commands in `.github/CI.md`.

1. Before merge, additive: create `signing` with a custom branch policy of
   `main`; copy the five values from Doppler into it. Repository secrets stay,
   so main keeps signing either way. Give `dev` a custom branch policy of
   `main` and delete the empty `pypi`.
2. Probe, before merge, from the PR branch (it already carries `environment:`
   on `notarization-check.yml`, and main's copy has `workflow_dispatch`):

   ```bash
   gh workflow run notarization-check.yml --ref <this branch>
   gh run watch
   ```

   It must fail with "Branch … is not allowed to deploy to signing due to
   environment protection rules" and run no step. If it runs, remove
   `deployment: false` and probe again.
3. Merge.
4. After one green `Packages (main)` and one green notarization check on main:
   delete the five repository secrets.
5. Run the acceptance check.

Not in these steps until someone answers (`scratch/questions.md`), because
neither can be undone: deleting the `dev` environment, which discards
`LFD_GITHUB_WEBHOOK_SECRET`; rotating the notary key or the certificate.

## Acceptance check

```bash
uv run pytest python/tests/test_workflow_secrets.py   # passes
uv run python scripts/check_actions_secrets.py        # prints nothing, exits 0
gh secret list                                        # none of the five signing names
```

The script and `gh secret list` are fully clean once LOO-1120 has deleted
`DEPENDABOT_MERGE_TOKEN`.

## Demo

The probe in step 2: the branch run is refused and no step runs; the same
command with `--ref main` goes green. Then add a throwaway repository secret
(`gh secret set PROBE --body x`), run the check script, read the line naming
it, delete it.

## Exclusions

- Rotating the certificate or notary key: a person's call, asked in
  `scratch/questions.md`.
- Pinning `uses:` to SHAs, Dependabot `cooldown`, `rust-cache` in release
  builds, the publisher's child environment (finding 4).
- Required reviewers on `signing`: they would stop per-commit signing.
- The cron host signing main builds so GitHub only compiles: `lf install --dev`
  reads the DMG from the Actions run, and the host cannot add an artifact to a
  finished run.

## Deferred findings

From the 2026-11-26 concept review (`scratch/concept-review.html`).

- **Local agent Sessions hold admin on the repository.** LOO-1118 scopes the
  token rentals get; local Sessions need the same: a token that can push and
  open PRs and cannot edit settings. Until then the admin row of the table
  stays.
- **The p12 and notary key are readable from the laptop's Doppler login.** They
  would move to a Doppler config the everyday login does not open, or leave
  Doppler once the cron host's keychain and the GitHub environment are both
  loaded. No GitHub setting changes this row.
- **The signing identity is hand-copied to three places** (Doppler, GitHub, the
  host keychain). The morning check catches a stale notary key and the next
  main commit a stale certificate. Fetching from Doppler by OIDC bound to
  `refs/heads/main` would remove the GitHub copy; whether the Doppler plan in
  use offers it is not verified. Worth a Task only if the copies drift.

## Review findings, placed

From `scratch/concept-review.html` as rewritten against the built change
(e67affb9c0).

- The promise named a holder and the ruleset on main makes it false: reworded
  to "a commit on main" in the Outcome and `.github/CI.md` (made for review).
- The check never reads an environment no workflow names, so `dev` passes:
  next pass.
- "The next release stops and names the change" overstated what a daily
  reading sees: `.github/CI.md` and the Outcome now say "while still in
  place". No record of uses added (made for review).
- One rule under two names; the preflight error calls every line a signing
  setting and says "differ" for a failed read: next pass.
- A required review on main, agents holding admin, the p12 in Doppler:
  separate outcomes; the last two are deferred above, the first is not
  proposed (it ends agents landing their own PRs).
- Deleting `dev`, rotation, the old Doppler token: questions, not plan steps.

## What goes

- `workflow_dispatch`, the `github.sha` fallbacks and the `event_name` clause
  in `main-packages.yml`.
- "credential-free" and "never enter … GitHub Actions" wording in
  `release/SCHEDULE.md` and `release.yml` where it is untrue.
- In settings: the five repository signing secrets; the empty `pypi`
  environment.

Must survive: signed, notarized `Loopflow-dmg` on every green main commit; the
daily notarization check and its issue; the daily release.

## Passes

- Built: synced onto LOO-1120; `environment: signing` on both jobs,
  `workflow_dispatch` and the `github.sha` fallbacks gone from
  `main-packages.yml`; `scripts/check_actions_secrets.py`, called by the
  release preflight; `python/tests/test_workflow_secrets.py`. The host's token
  reads the settings, so the preflight call stays.

- Next (one pass): the check reads every environment.
  - `read_settings` lists `environments` from GitHub (names, paginated) and
    reads each one's policy and secret names; it takes no argument.
    `Settings.environments` holds what exists, so a missing environment is an
    absent key and `Environment | None` goes.
  - `differences`: any environment with a secret and a policy other than
    `main` only is reported, named by a job or not; an environment a job names
    and GitHub lacks keeps "does not exist"; a name a job reads that is not
    set stays; "is set and no job names it" goes, with its test case.
  - `publish_release.check_release_host` raises "GitHub Actions secrets are
    not main-only (.github/CI.md, "Signing secrets")" with the script's lines.
    A failed read reaches the log as the script printed it.
  - Tests: an open environment no job names is reported by name
    (`environment dev: no branch policy by name; expected main only`); an
    empty open one is not; the canned `_gh` answers include the environment
    list.
  - `.github/CI.md`: the paragraph on what the script names, and the admin
    commands gain the `dev` policy.
  - Run it live; today it should print the seven lines plus `dev`. Teaches
    whether the host's `gh` can list environments (it read one by name).

Left, for a person with admin at ship-review ("Order of settings changes"):
create `signing`, copy the five values, give `dev` a `main` policy, run the
probe (settles `deployment: false`), merge, delete the repository secrets, run
the acceptance check.

Done when a branch run naming `signing` is refused, main still produces a
notarized DMG, none of the five names is a repository secret, and the release
preflight reports a setting that was changed.
