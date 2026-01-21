# publish-screenshot

Regenerate Maestro screenshots during publish and reference them in docs.

## What to build

1. `scripts/publish.py` runs screenshot generation during release PR creation
2. `docs/next/maestro.md` references the screenshots

## Key functions

```python
def _generate_screenshots() -> tuple[bool, str]:
    """Run scripts/generate_screenshots.py, return (success, output)."""
    ...

def _create_release_pr(...):
    # ... existing steps ...

    # Step 2.5: Generate screenshots (after tests, before version bump)
    print("Generating screenshots...")
    success, output = _generate_screenshots()
    if not success:
        print(f"Screenshot generation failed: {output}", file=sys.stderr)
        return 1

    # Include in release commit
    subprocess.run(["git", "add", "docs/*.png"], ...)
```

## Changes

### scripts/publish.py

Add `_generate_screenshots()` function:
- Runs `python scripts/generate_screenshots.py`
- Returns `(success, output)` like other publish helpers

Call it in `_create_release_pr()` between tests and version bump:
1. Run tests
2. Verify CI
3. **Generate screenshots** ← new
4. Generate release notes
5. Bump version, commit (now includes updated PNGs)

Add `--skip-screenshots` flag for machines without Maestro/demo repo setup.

### docs/next/maestro.md

Add image references:

```markdown
## Getting Started

![Maestro main window](../maestro-main.png)

### Installation
...

## Agents Panel

![Maestro with loops](../maestro-loops.png)

### Viewing Agents
...
```

## Constraints

- Screenshots require: Maestro built, `~/src/loopflow-demos` exists
- `--skip-screenshots` bypasses generation (for CI or minimal setups)
- Screenshots go in release commit, not separate commit

## Done when

```bash
# Screenshots referenced in docs
grep -q "maestro-main.png" docs/next/maestro.md

# Publish has screenshot flag
grep -q "skip-screenshots" scripts/publish.py

# Publish calls generate_screenshots
grep -q "_generate_screenshots" scripts/publish.py
```

All three pass.
