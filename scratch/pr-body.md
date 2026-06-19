## Try it!

```bash
uv run pytest python/tests/test_shell_installer.py -v
uv run pytest python/tests/
ruby -e 'require "yaml"; YAML.load_file(".github/workflows/release.yml"); puts "release.yml parsed"'
```

Expected results from this branch:

- shell installer regression tests: 7 passed
- Python test suite: 146 passed
- `release.yml parsed`

## Intent

Fix the release shell installer path where scripted installs could pass `--no-interactive` and have it interpreted as a version, producing a bad `v--no-interactive` release URL and a confusing follow-on failure. The installer now parses flags explicitly, treats `--no-interactive` as a compatibility no-op, and reports download failures at the point of failure.

## Assumptions

- `install.sh` should only install the `lf` and `lfd` binaries.
- Provider onboarding belongs to `lfd install`, where `--no-interactive` still controls prompts.
- Release workflow heredoc content is the published installer source of truth.

## Key decisions

- Preserve positional version installs while adding `--version <X>` / `--version=<X>`.
- Keep the shell POSIX-compatible; download to a file instead of relying on `pipefail`.
- Test the generated installer directly with stubbed `curl` and `tar` instead of adding a second installer copy.

## Not included

- No live GitHub release download test.
- No release packaging changes outside the installer heredoc.
