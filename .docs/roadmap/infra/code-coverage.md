# CI: Code Coverage Reporting

**Status:** proposed

## Problem

No visibility into test coverage. Don't know which code paths are tested, which are not. Hard to assess risk when making changes.

## Proposal

Add pytest-cov and report to Codecov:

1. Add `pytest-cov` to dev dependencies
2. Update CI to generate coverage report
3. Upload to Codecov for PR comments

```yaml
loopflow-test:
  runs-on: ubuntu-latest
  steps:
    - uses: actions/checkout@v4
    - uses: astral-sh/setup-uv@v4
    - run: uv sync
    - run: uv run pytest tests/ --cov=src/loopflow --cov-report=xml
    - uses: codecov/codecov-action@v4
      with:
        files: coverage.xml
```

## Why This Matters

- 506 tests exist but unknown coverage
- PR comments show coverage delta (regression protection)
- Visibility first, thresholds later

## Open Questions

- Codecov token setup for the repo
- Whether to add coverage threshold after establishing baseline
