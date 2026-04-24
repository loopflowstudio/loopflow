## Try it!

```bash
sed -n '1,120p' .github/dependabot.yml
sed -n '1,200p' .github/workflows/dependabot-auto.yml
sed -n '50,95p' TESTING.md

git diff --check
uv run python - <<'PY'
from pathlib import Path
import yaml

for path in [Path('.github/dependabot.yml'), Path('.github/workflows/dependabot-auto.yml')]:
    yaml.load(path.read_text(), Loader=yaml.BaseLoader)

workflow = yaml.load(Path('.github/workflows/dependabot-auto.yml').read_text(), Loader=yaml.BaseLoader)
assert workflow['on']['workflow_run']['workflows'] == ['CI']
assert set(workflow['jobs']) == {'enable-auto-merge', 'close-on-red'}
print('dependabot config + workflow parsed cleanly')
PY
```

Reviewer should see weekly updates configured for `uv`, `cargo`, `swift`, and `github-actions`, plus a zero-touch workflow that auto-merges green Dependabot PRs and closes red ones.

## Intent

Keep routine dependency bumps moving without human babysitting. Dependabot should open weekly update PRs, green ones should merge themselves, and red ones should get out of the way so the next bump can retry cleanly.

## Assumptions

- the main CI workflow remains named `CI`
- GitHub's default token can enable auto-merge, comment, and close PRs in this repo
- Dependabot PRs remain discoverable through `gh pr list --author app/dependabot`

## Key decisions

- match failed workflow runs back to PRs by head SHA instead of trying to infer PR numbers from `workflow_run`
- document the workflow-name coupling in `TESTING.md`, where repo maintainers already look for CI expectations
- keep the scope tight: weekly version updates plus zero-touch handling, without adding grouping or reviewer policy

## Not included

- Dependabot grouping rules, labels, or reviewer assignment
- retries or special cases for flaky CI
- any change to the human PR path or release workflows
