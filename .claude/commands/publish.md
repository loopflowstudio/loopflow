Publish the loopflow package to PyPI.

```bash
lf publish           # patch bump (default): 0.5.0 → 0.5.1
lf publish minor     # minor bump: 0.5.0 → 0.6.0 (substantial release)
lf publish major     # major bump: 0.9.0 → 1.0.0 (explicit full version bump only)
```

## Workflow

Execute these steps in order. Stop if any step fails.

### 1. Create release worktree

All changes—including version bumps—must happen on a branch, then merge to main. Create a worktree for the release:

```bash
wt switch --create release-vX.Y.Z
```

Use the version you'll be releasing (determined in step 3).

### 2. Run tests

```bash
uv run pytest tests/
```

If tests fail, stop and report which tests failed. Do not publish broken code.

### 3. Determine version bump

Read current version from `src/loopflow/__init__.py`. Then:

- If `patch`, `minor`, or `major` was passed as argument, use that
- Otherwise, analyze `git diff main...HEAD`:
  - **Patch** (default): Bug fixes, documentation, small improvements (0.5.0 → 0.5.1)
  - **Minor**: New features, substantial additions, significant new functionality (0.5.0 → 0.6.0)
  - **Major**: Only when user explicitly requests a full version bump (0.9.0 → 1.0.0)

Calculate new version (X.Y.Z):
- Patch: increment Z
- Minor: increment Y, reset Z to 0
- Major: increment X, reset Y and Z to 0

**Default to patch.** Most releases are small iterations. Minor bumps are for fairly substantial releases with significant new functionality. Major bumps require explicit user request.

### 4. Update version

Edit `src/loopflow/__init__.py`:
```python
__version__ = "X.Y.Z"
```

### 5. Generate release notes

Write `RELEASE_NOTES.md` (overwrite if exists):
```markdown
# vX.Y.Z

<2-3 sentence summary>

## Changes

- <notable change 1>
- <notable change 2>

## Breaking changes

<if major bump, list what breaks>
```

### 6. Commit and merge to main

Commit the version bump and release notes:
```bash
git add src/loopflow/__init__.py RELEASE_NOTES.md
git commit -m "release: vX.Y.Z"
git push -u origin HEAD
```

Merge to main using worktrunk:
```bash
wt land
```

### 7. Verify publish readiness

Switch to main and verify state using the publish helper:
```bash
cd /path/to/main/repo  # or wt switch main
uv run python -m loopflow.publish
```

This checks:
- On main branch
- Main is synced with origin/main
- No uncommitted changes

**Do not proceed unless the script reports "Ready to publish."**

### 8. Build and publish from main

Build and publish the package:
```bash
uv build
uv publish
```

Requires `UV_PUBLISH_TOKEN` env var or `~/.pypirc` credentials.

### 9. Install locally

```bash
uv tool install --force loopflow
```

### 10. Tag and push

```bash
git tag vX.Y.Z
git push --tags
```

## If something fails

- **Test failures:** Stop immediately. Report which tests failed.
- **Merge conflicts:** Resolve in the release worktree before landing.
- **Build failures:** Check `pyproject.toml`. Common issue: missing file in `[tool.hatch.build]`.
- **Publish failures:** Check `UV_PUBLISH_TOKEN`. May need to generate new token at pypi.org.
- **"Not ready" from publish script:** Follow the message to fix (sync main, commit changes, etc.)
- **Don't leave partial state.** If publish fails after version bump, you may need to manually increment again.

## Output

Report each step as it completes. End with:
- New version number
- PyPI URL: https://pypi.org/project/loopflow/

## Auto mode

In auto/headless runs, do not pause to ask questions. Make the best assumption you can and append any open questions to `.design/questions.md`.
