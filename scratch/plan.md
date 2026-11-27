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

The change is committed as `0586cdf9f4` on `jack-heart/remove-dependabot`
(base `472bacbd64`): four files, 4 insertions, 163 deletions.

- `.github/dependabot.yml` deleted (weekly uv, cargo, swift, github-actions).
- `.github/workflows/dependabot-auto.yml` deleted (`pull_request_target`
  auto-merge with `DEPENDABOT_MERGE_TOKEN`, close-on-red).
- `.github/CI.md`: Dependabot section gone, one mention reworded.
- `scripts/testing.md`: table row no longer names Dependabot.

What remains: review, PR, land, then the cleanup outside the repository.

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
- **No other updater exists.** No Renovate config, no scheduled
  `cargo update` or `uv lock --upgrade` under `.github/` or `scripts/`.
- **The token lives in two places**, not one: the `DEPENDABOT_MERGE_TOKEN`
  Actions secret (`gh secret list`) and a Doppler secret of the same name
  (`doppler secrets --only-names`), plus the GitHub PAT both hold.
- **Five stale branches on origin**: `dependabot/cargo/fancy-regex-0.20.0`,
  `dependabot/cargo/fancy-regex-0.20.1`, `dependabot/cargo/jsonschema-0.61.2`,
  `dependabot/cargo/rusqlite-0.43.0`,
  `dependabot/github_actions/actions/download-artifact-8`.

## Decision (made for review)

Remove, don't tame. Passed over: keeping Dependabot with a `cooldown`, no
auto-merge and required review. That keeps a write-scoped PAT and a
`pull_request_target` workflow alive to save a `cargo update` now and then, and
the brief already chose removal.

## Acceptance check

After merge, on `main`:

```bash
git grep -il dependabot -- ':!release'              # no output
ls .github/dependabot.yml .github/workflows/dependabot-auto.yml   # both: No such file
gh workflow list --all | grep -i dependabot         # no output, or disabled/absent
gh pr list --author app/dependabot --state open     # empty, and still empty a week on
```

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
- Workflow actions pinned by mutable tag (`actions/checkout@v7`,
  `astral-sh/setup-uv@v7`, `dtolnay/rust-toolchain@stable`,
  `Swatinem/rust-cache@v2`, `taiki-e/install-action@nextest`, …) still move
  when upstream moves the tag. That is a second automatic-update path this
  Task leaves alone; see `scratch/questions.md`.
- A written procedure or cadence for hand bumps. "By hand" means an ordinary
  Task; nothing new is documented.
- Turning off Dependabot alerts.

## What goes

Already gone in `0586cdf9f4`: both files and the docs above. Survives: CI,
merge queue, release and package workflows, untouched.

## Pass (one)

1. Review `0586cdf9f4`. One cosmetic nit to fix in passing: the reworded
   paragraph in `.github/CI.md` (lines 14–17) leaves a short line, "CI runs the
   suites and"; reflow it.
2. PR and land. No build or test: the diff is YAML deletions and prose. CI's
   own run on the PR shows the remaining workflows still parse.
3. After merge, outside the repository (irreversible, so a person does these
   or says to):
   - revoke the GitHub PAT behind `DEPENDABOT_MERGE_TOKEN`;
   - `gh secret delete DEPENDABOT_MERGE_TOKEN`;
   - delete `DEPENDABOT_MERGE_TOKEN` from Doppler;
   - delete the five `dependabot/*` branches on origin.

## Done when

A week passes with no Dependabot PR, the token is revoked and absent from both
stores, and origin has no `dependabot/*` branch.
