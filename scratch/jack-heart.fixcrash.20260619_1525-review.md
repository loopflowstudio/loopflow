# Gate review: installer non-interactive crash

## What was implemented

The release-generated `install.sh` now parses installer flags instead of treating the first argument as the version unconditionally. `--no-interactive` is accepted as a no-op because the shell installer only downloads `lf` and `lfd`; provider onboarding remains in `lfd install`.

The installer also downloads the release tarball to a temporary file before extraction. That makes a failed `curl` stop with a direct download error instead of falling through to a later, cryptic missing-file failure.

Regression coverage now extracts the installer heredoc from `.github/workflows/release.yml` and runs it with stubbed `curl` and `tar` binaries.

## Key choices

- Kept `--no-interactive` as a compatibility no-op rather than rejecting it. Existing CI and scripted installs may already pass the flag from the older README copy.
- Added `--version <X>` and `--version=<X>` parsing while preserving positional `VERSION`.
- Used a temporary tarball instead of `curl | tar` because POSIX `sh` has no portable `pipefail`.
- Tested the generated heredoc directly instead of duplicating installer logic in a separate script.

## How it fits together

The release workflow remains the source of truth for the published shell installer. Python tests read that heredoc, write it to a temp file, and replace external commands with local stubs so parser and error behavior can be verified without network access or real archive extraction.

## Risks and bottlenecks

- The installer still lives inside YAML, so heredoc indentation remains important. The syntax test catches malformed shell, and the extraction helper catches a missing heredoc terminator.
- The tests validate URL construction and control flow, not a real GitHub release download.
- No branch design doc existed under `scratch/`, so validation used the regression behavior implied by the change.

## What's not included

- No changes to release artifact creation or binary packaging.
- No live network install test.
- No changes to provider onboarding beyond README clarification that `lfd install` owns it.

## Validation

```bash
uv run ruff check python/tests/test_shell_installer.py
uv run pytest python/tests/test_shell_installer.py -v
ruby -e 'require "yaml"; YAML.load_file(".github/workflows/release.yml"); puts "release.yml parsed"'
uv run pytest python/tests/
```

Results:

- `ruff`: passed
- shell installer regression tests: 7 passed
- `release.yml` YAML parse: passed
- Python test suite: 146 passed
