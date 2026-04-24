# Review

## What was implemented

Added weekly Dependabot version updates for the repo's package managers and a GitHub Actions workflow that keeps Dependabot PRs zero-touch. Documented the automation in `TESTING.md`, including the coupling to the existing `CI` workflow name.

## Key choices

- Used one Dependabot entry per ecosystem already present in the repo: `uv`, `cargo`, `swift`, and `github-actions`
- Enabled auto-merge only for Dependabot-authored PRs on `pull_request_target`, with no checkout or untrusted code execution
- Matched failed CI runs back to the open PR by head SHA because `workflow_run` does not expose the PR number directly
- Documented the `workflow_run.workflows: ["CI"]` dependency where maintainers already look for CI expectations: `TESTING.md`

## How it fits together

Dependabot opens weekly PRs from `.github/dependabot.yml`. `.github/workflows/dependabot-auto.yml` turns on squash auto-merge when a Dependabot PR opens or reopens, then watches completed `CI` pull-request runs and closes the matching PR if the run fails. `TESTING.md` explains the automation and the one piece of repo coupling maintainers must preserve.

## Risks and bottlenecks

- `close-on-red` depends on the CI workflow continuing to be named `CI`
- The workflow assumes Dependabot PRs appear in `gh pr list` as author `app/dependabot`
- Validation here is YAML parsing plus shape checks; GitHub still performs the final workflow semantic validation when the files land

## What's not included

- no custom labels, reviewers, or grouping rules for Dependabot PRs
- no special handling for flaky CI; red Dependabot PRs close and wait for the next bump
- no changes to human-authored PR flow, merge queue behavior, or release automation

## Validation

```bash
git diff --check
uv run python - <<'PY'
from pathlib import Path
import yaml

files = [Path('.github/dependabot.yml'), Path('.github/workflows/dependabot-auto.yml')]
for path in files:
    yaml.load(path.read_text(), Loader=yaml.BaseLoader)

workflow = yaml.load(Path('.github/workflows/dependabot-auto.yml').read_text(), Loader=yaml.BaseLoader)
assert workflow['on']['workflow_run']['workflows'] == ['CI']
assert set(workflow['jobs']) == {'enable-auto-merge', 'close-on-red'}
PY
```
