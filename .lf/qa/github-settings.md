Read when the change touches GitHub Actions secrets, environments, branch policies or `scripts/check_actions_secrets.py`.

# GitHub settings

Never try a settings change on `loopflowstudio/loopflow`: its `signing`
environment holds the Developer ID certificate, and a job that names a missing
environment creates it open. Use the private probe repository
`loopflowstudio/actions-settings-probe` (the organization's plan gives private
repositories environments and branch policies). Local `gh` logins lack
`delete_repo`, so the repository stays; reuse it.

It holds `scripts/check_actions_secrets.py` (copy the branch's over it),
`.github/workflows/probe.yml` on `main` (a `workflow_dispatch` job naming
`signing` with `deployment: false` that prints `${#PROBE_VALUE}`), and on the
`side` branch `bare.yml` (`on: push`, the same read with no environment) plus
`pull_request` on `probe.yml`. Environments `signing` and `dev` allow `main`
only and hold `PROBE_VALUE` and `WEBHOOK_SECRET`, dummy values.

```bash
git clone https://github.com/loopflowstudio/actions-settings-probe /tmp/probe && cd /tmp/probe
cp <checkout>/scripts/check_actions_secrets.py scripts/
<checkout>/.venv/bin/python scripts/check_actions_secrets.py   # prints nothing
gh workflow run probe.yml --ref side    # refused, no step, no runner
gh workflow run probe.yml --ref main    # PROBE_VALUE length: 11
```

The script reads `repos/{owner}/{repo}` from the directory it sits in, so the
copy inside the probe clone reads the probe's settings. The refusal is an
annotation, not a log line:

```bash
job=$(gh api repos/{owner}/{repo}/actions/runs/<run>/jobs --jq '.jobs[0].id')
gh api repos/{owner}/{repo}/check-runs/$job/annotations --jq '.[].message'
```

Each line of the check has one setting behind it: `gh secret set PROBE`
(repository secret); `POST …/environments/signing/deployment-branch-policies`
with `name=release/*` or `type=tag` (widened); `PUT …/environments/signing`
with `protected_branches=true` or `{"deployment_branch_policy":null}` (no
policy by name); `gh secret delete PROBE_VALUE --env signing` (named and not
set). Put each back. Leaving custom policies and returning drops the `main`
rule: the check prints `allows no branch` until it is posted again.

Seen 2026-11-27: a branch dispatch and a pull request naming `signing` are both
refused with `deployment: false` set, and no deployment record is written; a
`PUT` that adds a branch policy keeps the environment's secrets.

The release preflight is `doppler run -p loopflow -c prd -- uv run python
scripts/publish_release.py check`. It is read-only and stops at the settings
check while the real repository's settings differ. To run the check with the
release host's `gh` login, copy the script and `.github/workflows` to a temp
directory on `mini-heart`, `git init` it with the repository as `origin`, and
run `uv run --no-project --with pyyaml python scripts/check_actions_secrets.py`.

Not reachable with one account: a pull request from a fork's branch named
`main`, which is what `event == 'push'` in `main-packages.yml` stops.
