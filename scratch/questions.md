# Open questions (LOO-1117)

Assumptions taken with nobody in the conversation; each is reversible unless
marked.

- The Task's promise is written by holder: push access cannot sign; an admin
  token still can, with one API call the next release preflight reports.
  Alternatives: "a branch cannot sign"; hold until agents stop holding admin.
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
