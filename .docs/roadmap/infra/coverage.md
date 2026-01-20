---
status: proposed
area: infra
---

# Add code coverage reporting

The test suite has ~7500 lines of tests across 27 files but no coverage tracking. We don't know what's tested and what isn't.

## Scope

Included:
- Add pytest-cov to dev dependencies
- Report coverage in CI
- Upload to Codecov or similar for visibility
- Add coverage badge to README

Not included:
- Coverage thresholds that fail CI (start with visibility only)
- Per-file coverage requirements
- Branch coverage (line coverage first)

## Approach

1. Add `pytest-cov` to dev dependencies in pyproject.toml
2. Update CI to run `uv run pytest tests/ --cov=src/loopflow --cov-report=xml`
3. Add Codecov action to upload results:

```yaml
- uses: codecov/codecov-action@v4
  with:
    files: ./coverage.xml
```

4. Add badge to README once reporting is live

Start with visibility—see what's covered. Add enforcement later once we have a baseline.
