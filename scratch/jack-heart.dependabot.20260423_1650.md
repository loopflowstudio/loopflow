# Dependabot zero-touch

Keep weekly dependency PRs moving without manual babysitting.

## Scope

- add Dependabot version updates for the package managers this repo already uses
- document how the zero-touch workflow interacts with the existing `CI` workflow
- leave release flow and human-authored PR flow unchanged

## Done when

- `.github/dependabot.yml` schedules weekly updates for `uv`, `cargo`, `swift`, and `github-actions`
- `.github/workflows/dependabot-auto.yml` enables auto-merge for Dependabot PRs and closes red PRs after CI fails
- `TESTING.md` explains the automation and the `CI` workflow-name coupling maintainers need to preserve
- both YAML files parse cleanly
