Read when the change touches `.github/workflows/` or the actions they pin.

A PR with notes in `scratch/` is a checkpoint: `scratch-clear` and
`macos-inputs` run and every other job skips, so its green check says nothing
about a workflow edit. To run the edited workflows before landing, push a copy
of the branch with the added `scratch/` files removed and open a draft PR:

```bash
export GIT_INDEX_FILE=$(mktemp -u /tmp/qaidx.XXXX)
git read-tree HEAD
for f in $(git diff --name-only --diff-filter=A origin/main HEAD -- scratch); do
  git update-index --force-remove "$f"
done
c=$(git commit-tree "$(git write-tree)" -p HEAD -m 'QA only: clear scratch')
rm -f "$GIT_INDEX_FILE"; unset GIT_INDEX_FILE
git push origin "${c}:refs/heads/<branch>-qa-run"     # braces: zsh reads $c:r as a modifier
gh pr create --draft --head <branch>-qa-run --base main --title 'QA run for #<pr> (do not merge)' --body '…'
gh run watch <run id> --exit-status
```

The checkout and its index are untouched. Close the PR with
`gh pr close <n> --delete-branch` when the run ends.

- `pull_request` runs use the PR's own workflow files, so the copy runs the
  edit. Scheduled workflows with `workflow_dispatch` run from a branch too:
  `gh workflow run architecture-drift.yml --ref <branch>-qa-run` only reads and
  uploads a report.
- Not reachable before merge: `main-packages.yml`, `package-build.yml`,
  `macos-main.yml` and `release.yml`. They run from `main` or a release tag,
  sign with the Developer ID certificate or publish. Dispatching
  `main-packages.yml` from a branch would publish a dev package; don't.
- `gh run view <id> --log | grep 'Download action repository'` lists the
  commit each job fetched for every action.
- A pin is right when `gh api repos/<owner>/<action>/commits/<tag> --jq .sha`
  prints it. `actionlint` (Homebrew) checks the workflow files locally.
