# Review

## What was implemented

Added weekly Dependabot updates for the repo's four dependency surfaces: `uv`, `cargo`, Swift Package Manager, and GitHub Actions.

Added `.github/workflows/dependabot-auto.yml` so Dependabot PRs handle themselves: green PRs get squash auto-merge on open or reopen, and PRs tied to failed `CI` runs get a comment and close themselves.

Updated `TESTING.md` with the maintainer-facing contract for this automation, including the workflow-name coupling that keeps the close-on-red path working.

## Key choices

- Match failed runs back to PRs by `workflow_run.head_sha`. GitHub does not hand the PR number directly to `workflow_run`, so the workflow looks up the open Dependabot PR whose head SHA matches the failed run.
- Keep the docs in `TESTING.md`. Maintainers already use that file to understand CI expectations, so the Dependabot workflow contract lives beside the CI docs it depends on.
- Keep the policy narrow: weekly updates plus zero-touch merge-or-close behavior. Grouping, labels, reviewers, and retry policy stay out of scope.

## How it fits together

Dependabot opens weekly PRs from `.github/dependabot.yml`. When one opens or reopens, `pull_request_target` enables squash auto-merge. Later, if the `CI` workflow finishes with a failure for a pull-request run, the `workflow_run` handler looks up the matching Dependabot PR by head SHA, comments, and closes it so the next weekly bump can try again.

## Risks and bottlenecks

- The close-on-red path depends on the main CI workflow still being named `CI`.
- The workflow assumes `GITHUB_TOKEN` in this repo can enable auto-merge, comment, and close PRs.
- `gh pr list --author app/dependabot` must keep surfacing Dependabot PRs the same way for the SHA lookup to work.

## What's not included

- Dependabot grouping rules, labels, or reviewer assignment
- Special handling for flaky CI or selective retries
- Any change to the human PR path or release workflows

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
