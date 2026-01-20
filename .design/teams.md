# GitHub CI + Merge Queue

## What to build

GitHub Actions workflow that runs tests on PRs and merge queue. Update `lfops land` to use merge queue instead of direct merge.

---

## Changes

### 1. GitHub Actions workflow

```yaml
# .github/workflows/ci.yml
name: CI

on:
  pull_request:
  merge_group:

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: astral-sh/setup-uv@v4
      - run: uv sync
      - run: uv run pytest tests/
```

Runs on:
- `pull_request` — when PR is opened or updated
- `merge_group` — when PR enters merge queue

### 2. Branch protection rules

Configure via GitHub UI or `gh` CLI:
- Require status checks: `test` job must pass
- Require merge queue
- Require branches to be up to date (merge queue handles this)

### 3. `lfops land` update

Change from direct merge to auto-merge (queues the PR):

```python
# Before
merge_cmd = ["gh", "pr", "merge", str(pr_number), "--squash", "--subject", title]

# After
merge_cmd = ["gh", "pr", "merge", str(pr_number), "--squash", "--auto", "--subject", title]
```

The `--auto` flag enables auto-merge. GitHub merges the PR when:
1. All required checks pass
2. PR enters and exits merge queue successfully

### 4. Move publish script to lfops (optional)

Move `scripts/publish.py` logic into `lfops publish` for consistency. Same behavior, better CLI integration.

### 5. Documentation updates

Update docs that describe `lfops land` behavior:

**`docs/lfops.md`** — update `lfops land` description:
```markdown
## lfops land

Submit PR to merge queue, cleanup worktree after merge.

```bash
lfops land
```

Enables auto-merge on your PR. GitHub merges when CI passes and the merge queue clears. After merge, deletes the remote branch and removes the local worktree.
```

**`docs/patterns.md`** — update workflow example to mention CI:
```markdown
## PR Workflow

```bash
lfops pr      # create or update PR, CI runs
lfops land    # submit to merge queue, merges when CI passes
```
```

**`docs/getting-started.md`** — add note about CI:
```markdown
## Ship Your Work

```bash
lfops pr      # create PR (CI runs automatically)
lfops land    # submit to merge queue
```
```

---

## What stays the same

- Publishing remains manual for now
- Local development workflow unchanged
- Prompts and tasks unchanged

---

## Follow-on: Cron publisher agent

After MVP, add a daemon agent that publishes on a schedule:

```yaml
# ~/.lf/agents/publisher.md
---
repo: /path/to/loopflow
trigger:
  kind: cron
  cron: "0 9 * * *"
---

Check if there are commits on main since the last release tag.
If yes, run `lfops publish patch`.
```

Runs locally via `lfd`, not in GitHub CI. Keeps publishing under your control while automating the "did I forget to release?" check.

---

## Constraints

- **Merge queue, not just branch protection.** Branch protection blocks merge until CI passes but doesn't re-run tests after rebase. Merge queue re-tests after rebasing onto latest main.
- **Admin bypass for hotfixes.** Repo admins can bypass branch protection. If CI is broken, you need to be able to land the fix without CI blocking you.
- **Keep CI simple.** Just pytest for now. Lint/typecheck can come later.
- **No macOS CI for Maestro yet.** DMG builds stay local. macOS runners are slow/expensive.

---

## Done when

```bash
# Create a PR
git checkout -b test-ci
echo "# test" >> README.md
git add -A && git commit -m "test"
git push -u origin test-ci
gh pr create --title "Test CI" --body "Testing merge queue"

# Verify CI runs
gh pr checks  # should show "test" job running/passing

# Land via merge queue
lfops land
# → PR enters merge queue (not immediate merge)
# → CI runs in merge queue context
# → Merges when green
```

