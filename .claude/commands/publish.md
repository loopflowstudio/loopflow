Publish the loopflow package to PyPI.

```bash
lf publish           # auto-detect version bump from changes
lf publish minor     # force minor version bump
lf publish major     # force major version bump
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

- If `minor` or `major` was passed as argument, use that
- Otherwise, analyze `git diff main...HEAD`:
  - **Major**: Breaking changes to CLI flags, removed commands, incompatible config changes
  - **Minor**: New commands, new flags, non-breaking additions (default)

Calculate new version (X.Y.Z):
- Minor: increment Y, reset Z to 0
- Major: increment X, reset Y and Z to 0

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

