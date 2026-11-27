# Open questions (LOO-1117)

Assumptions taken with nobody in the conversation; each is reversible unless
marked.

- The sync keeps this Task's plan, review, and questions at their existing
  paths. LOO-1120 keeps its notes on its own branch.
- The Task's promise is "nothing is signed as Loopflow without a commit on
  main". Push access can still sign by landing its own PR: main asks for the
  merge queue and `tests-result`, no review. Alternative: keep "push access is
  not enough" with a corrected holder table.
- `dev` gets a `main` branch policy in the settings steps, and the check stops
  reporting a secret no job names. Alternative: the daily release stops on
  `dev` until someone decides whether to delete it.
- No record of each use of `signing` (`deployment: false` stays), so a policy
  widened, used and put back before 10:00 is not seen. Alternative: drop the
  key and read the Deployments list.
- No `dependabot` environment: LOO-1120 (#2054) deletes that workflow and its
  token, and this builds on it. If #2054 stalls, `DEPENDABOT_MERGE_TOKEN`
  stays a branch-readable repository secret until it lands.
- The release preflight on the cron host runs
  `scripts/check_actions_secrets.py`, so a changed setting stops that day's
  release. Alternatives: no script; a script run by hand.
- `workflow_dispatch` on `main-packages.yml` is dropped rather than guarded on
  `refs/heads/main`.

Needs a person; not in the plan's steps until answered:

- **Cannot be undone.** Delete environment `dev`? It holds
  `LFD_GITHUB_WEBHOOK_SECRET` (2026-02-13), whose value cannot be read back;
  `lfd` left the repository on 2026-09-27. Does anything outside the
  repository still use that webhook? (`pypi` is empty and is deleted.)
- **Cannot be undone.** Rotate the signing identity? The secrets were readable
  by anything with push access from 2026-10-30 to 2026-11-26. The one branch
  dispatch on record (run 41111152406, 2026-10-30) is the LOO-688 feature
  branch; runs of branch workflows since deleted were not counted. The review
  recommends rotating the notary API key only (a new key in App Store Connect,
  one Doppler edit, nothing shipped affected). Revoking the certificate
  reaches DMGs already downloaded.
- `DOPPLER_TOKEN_PRD` was already absent from repository secrets on
  2026-11-26, and `loopflow`/`prd` lists no service tokens now. Was the token
  behind it revoked? Doppler's activity log answers it.

From writing the docs (reversible):

- The docs give a rule for later secrets: every CI secret lives in an
  environment with a branch policy, and `gh secret set` is never run without
  `--env`. The plan only said "there are no repository secrets".
- The check script's lines are worded in `.github/CI.md` before the script
  exists (`repository secret NAME: …`, `environment signing: …`). The build
  can change the wording; the doc follows.
- The holder table in `.github/CI.md` says the Doppler login can sign. Is that
  row wanted in a checked-in public file?

From the build (2026-11-26):

- The cron host's `gh` login reads repository secret names, environment secret
  names and branch policies (checked over SSH against `github-pages`), so the
  preflight calls the check. Its `gh` is too old for `--slurp`; the script
  reads names with `--paginate --jq`.
- The repository belongs to an organization, so the check also reports
  organization secrets shared with it (none today). Not in the plan.
- From merge until the six repository secrets are deleted (five signing,
  `DEPENDABOT_MERGE_TOKEN`), the release preflight stops the daily release.
  Today's live run prints those six lines and `environment signing: does not
  exist; the first job naming it creates it open`.
- `actionlint` accepts `deployment: false`. Whether the branch policy still
  refuses with it is the probe at ship-review, not run here: the environment
  does not exist yet and creating it is an admin settings step.

From the second build (2026-11-26):

- `PUT environments/dev` with a custom branch policy is written in
  `.github/CI.md` from GitHub's API shape and was not run here (admin settings
  step). Whether the PUT leaves `dev`'s existing secret in place is expected
  and unverified.
- An environment set to "protected branches only" prints `no branch policy by
  name`, the same line as an open one: main's ruleset is not a classic branch
  protection, so that setting is not read as `main` only.
- After the settings steps the preflight still stops the daily release on
  `DEPENDABOT_MERGE_TOKEN` until LOO-1120 deletes it, and on `dev` until its
  policy is set.

From the second review (2026-11-26):

- The lint no longer pins the secret-reading jobs or the name `signing`; a
  third job reading from its own main-only environment passes. Alternative:
  keep the list so a new reader is always a test edit in its PR.
- The fork-PR walk (a fork's branch named `main` fires `workflow_run`, the
  run is on ref `main`, `event == 'push'` skips it) is from GitHub's docs and
  was not run against this repository.

From QA (2026-11-27):

- A private repository, `loopflowstudio/actions-settings-probe`, was created
  to run the settings steps and the probe away from the signing keys. It holds
  dummy values only. The local `gh` login lacks `delete_repo`, so it stays
  until someone deletes it; `.lf/qa/github-settings.md` describes reusing it.
- Answered there: the branch policy refuses with `deployment: false`, and the
  `PUT` that restricts `dev` keeps its secret. Seen on a private repository on
  the organization's Team plan, not on the public one.
- A failed read in the preflight still prints under "GitHub Actions secrets
  are not main-only", followed by `could not read …`. Alternative: its own
  heading.
