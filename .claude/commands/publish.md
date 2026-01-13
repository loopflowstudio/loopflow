Publish the loopflow package to PyPI.

```bash
lf publish           # patch bump (default): 0.5.0 → 0.5.1
lf publish minor     # minor bump: 0.5.0 → 0.6.0 (substantial release)
lf publish major     # major bump: 0.9.0 → 1.0.0 (explicit full version bump only)
```

## Workflow

Execute these steps in order. Stop if any step fails.

### 1. Run tests
```bash
uv run pytest tests/
```
If tests fail, stop and report which tests failed. Do not publish broken code.

### 2. Determine version bump

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

### 3. Update version
Edit `src/loopflow/__init__.py`:
```python
__version__ = "X.Y.Z"
```

### 4. Generate release notes
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

### 5. Build and publish
```bash
uv build
uv publish
```
Requires `UV_PUBLISH_TOKEN` env var or `~/.pypirc` credentials.

### 6. Install locally
```bash
uv tool install --force loopflow
```

### 7. Commit and tag
```bash
git add src/loopflow/__init__.py RELEASE_NOTES.md
git commit -m "release: vX.Y.Z"
git tag vX.Y.Z
git push && git push --tags
```

## If something fails

- **Test failures:** Stop immediately. Report which tests failed.
- **Build failures:** Check `pyproject.toml`. Common issue: missing file in `[tool.hatch.build]`.
- **Publish failures:** Check `UV_PUBLISH_TOKEN`. May need to generate new token at pypi.org.
- **Don't leave partial state.** If publish fails, revert the version bump before stopping.

## Output

Report each step as it completes. End with:
- New version number
- PyPI URL: https://pypi.org/project/loopflow/

## Auto mode

In auto/headless runs, do not pause to ask questions. Make the best assumption you can and append any open questions to `.design/questions.md`.

