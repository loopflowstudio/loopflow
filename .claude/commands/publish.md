Build and publish the loopflow Python package to PyPI.

Run with version type argument to override automatic detection:
```bash
lf publish minor    # force minor version bump
lf publish major    # force major version bump
```

Without an argument, version type is determined from the changes.

## Process

1. **Run tests**
   - Run `uv run pytest tests/` and ensure all tests pass
   - If tests fail, stop and report the failures

2. **Determine version bump**
   - If `minor` or `major` was passed as argument, use that
   - Otherwise, analyze `git diff main...HEAD` to decide:
     - **Major**: Breaking changes to public API, removed features, incompatible config changes
     - **Minor**: New features, new commands, non-breaking additions (default for most changes)
   - Read current version from `src/loopflow/__init__.py`
   - Calculate new version (X.Y.Z format: major.minor.patch)
   - For minor: increment Y, reset Z to 0
   - For major: increment X, reset Y and Z to 0

3. **Update version**
   - Edit `src/loopflow/__init__.py` to set `__version__ = "X.Y.Z"`

4. **Generate release notes**
   - Run `git log main..HEAD --oneline` for commit history
   - Run `git diff main...HEAD` for full changes
   - Write release notes to `RELEASE_NOTES.md` (overwrite if exists):
     - Version number and date
     - Summary of what changed (2-3 sentences)
     - Notable changes as bullet points
     - Breaking changes section if major bump

5. **Build package**
   - Run `uv build` to create distribution files in `dist/`
   - Verify build succeeded

6. **Publish to PyPI**
   - Run `uv publish` to upload to PyPI
   - This requires PyPI credentials configured (via `~/.pypirc` or `UV_PUBLISH_TOKEN`)

7. **Install locally**
   - Run `uv tool install --force loopflow` to install the published version
   - Or `pip install --upgrade loopflow` as fallback

8. **Commit and tag**
   - Stage the version change and release notes
   - Commit with message: `release: vX.Y.Z`
   - Create git tag: `git tag vX.Y.Z`
   - Push commit and tag: `git push && git push --tags`

## Output

Report each step as it completes. End with:
- The new version number
- Link to PyPI package page: https://pypi.org/project/loopflow/
- Any warnings or issues encountered

## If something fails

- Test failures: Stop and report which tests failed
- Build failures: Check pyproject.toml configuration
- Publish failures: Check PyPI credentials, may need `UV_PUBLISH_TOKEN` env var
- Don't leave partial state (uncommitted version bump, untagged release)

## Auto mode

In auto/headless runs, do not pause to ask questions. Make the best assumption you can and append any open questions to `.design/questions.md`.

