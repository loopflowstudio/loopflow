# lfops publish

## What to build

Add `lfops publish` command that builds, publishes to PyPI, and updates the local installation—mostly deterministic Python code with one LLM call for release notes.

## Current state

- `publish.py` exists with helper functions: `get_version`, `bump_version`, `check_publish_ready`, `run_tests`, `build_package`, `publish_package`, `install_locally`, `create_release_commit`
- `.claude/commands/publish.md` is an LLM-driven workflow prompt
- **URGENT:** v0.6.0 on PyPI has merge conflicts in `__init__.py`—package is broken and can't import. Need to publish a fix (v0.6.1)

## Data structures

```python
# In llm_http.py
class ReleaseNotes(BaseModel):
    summary: str      # 2-3 sentences
    changes: list[str]  # bullet points
```

## Key functions

```python
# In llm_http.py
def generate_release_notes(repo_root: Path, old_version: str, new_version: str) -> ReleaseNotes:
    """Generate release notes from commits since last tag via API."""

# In lfops.py
@app.command()
def publish(
    bump: str = typer.Argument("patch", help="Version bump: patch, minor, or major"),
    dry_run: bool = typer.Option(False, "--dry-run", "-n", help="Show what would be done"),
    skip_tests: bool = typer.Option(False, "--skip-tests", help="Skip test run"),
) -> None:
    """Build, publish to PyPI, and install locally."""
```

## Release notes prompt

Add `src/loopflow/prompts/RELEASE_NOTES.md`:

```markdown
Generate release notes for a version bump.

You're writing for someone scanning PyPI or GitHub releases to decide if they should upgrade. Outcome over process. What's new, not how it was built.

## Input

You'll receive:
- Commits since the last release tag
- The version bump (old → new)

## Output format

Return a structured response with:
- **summary**: 2-3 sentences explaining what this release adds or fixes
- **changes**: bullet list of notable changes (3-8 items)

## Style

Lead with what users get, not what changed internally.

Good:
- "Add `lfops publish` command for automated PyPI releases"
- "Fix session tracking when daemon isn't running"

Bad:
- "Refactored publish.py to use new pattern"
- "Updated llm_http.py with release notes generation"

Skip internal refactors unless they affect behavior. Focus on user-visible changes.
```

## Workflow

The command runs these steps in order, stopping on any failure:

1. **Preflight checks**
   - Must be on main branch
   - Main must be synced with origin/main
   - No uncommitted changes
   - uv and git available

2. **Run tests** (unless `--skip-tests`)
   - `uv run pytest tests/`
   - Fail if tests fail

3. **Version bump**
   - Read current version from `__init__.py`
   - Calculate new version based on bump type
   - Write new version to `__init__.py`

4. **Generate release notes** (LLM call via Anthropic API)
   - Get commits since last tag: `git log v{old}..HEAD --oneline`
   - Call `generate_release_notes()` with pydantic-ai Agent (like commit messages)
   - Write `RELEASE_NOTES.md` in markdown format

5. **Commit release**
   - `git add src/loopflow/__init__.py RELEASE_NOTES.md`
   - `git commit -m "release: vX.Y.Z"`
   - `git push`

6. **Tag**
   - `git tag vX.Y.Z`
   - `git push --tags`

7. **Build**
   - `uv build`
   - Check dist/ contains expected files

8. **Publish**
   - `uv publish`
   - Requires `UV_PUBLISH_TOKEN` env var

9. **Install locally**
   - `uv tool install --force loopflow`
   - Verify `lf --version` shows new version

## Dry run mode

With `--dry-run`, print what would happen at each step without executing:

```
Would bump version: 0.6.0 → 0.6.1 (patch)
Would run tests
Would generate release notes
Would commit: release: v0.6.1
Would tag: v0.6.1
Would build package
Would publish to PyPI
Would install locally
```

## UI changes

None. This is a CLI command only.

## Constraints

- Must run from main branch only (no worktree flow)
- Must not leave partial state on failure
- LLM call is for release notes only—everything else is deterministic
- Requires `UV_PUBLISH_TOKEN` environment variable

## Done when

```bash
# Basic validation
lfops publish --help | grep -q "patch, minor, or major"

# Dry run works
lfops publish --dry-run

# After a real publish:
lf --version  # shows new version
pip index versions loopflow | grep "new_version"
```
