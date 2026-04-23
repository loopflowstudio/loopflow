# Regression tier

Expensive end-to-end tests that exercise live lfd against real filesystems and
real tmux. Each test reproduces a user-visible bug that previously shipped to
production, so the suite is also a living changelog of "things we've promised
won't break again."

## Running

```bash
uv run pytest tests/regression/ -v                # all regression tests
uv run pytest tests/regression/test_run_with_roadmap_item_on_pm_wave.py -v
```

The suite takes minutes, not seconds — each test spins up a fresh `lfd`
process in an isolated temp `HOME`. Runs nightly in CI via
[`regression-daily.yml`](../../.github/workflows/regression-daily.yml); not
gated on PRs.

## When to add a test here

Add one any time you ship a fix where:

- The bug only shows up against a live daemon or subprocess (tmux, git,
  PM HTTP calls).
- A cheaper unit test would require reshaping production code to be testable.
- The failure mode is a full outage (panic, empty reply, stuck wave) that
  per-PR CI can't detect.

Don't add here if the fix is covered by a unit test or Swift state test —
those run on every PR and are faster. The regression tier is for bugs that
need a real runtime to notice.

## Adding a test

1. Create `tests/regression/test_<scenario>.py`.
2. Mark it with `pytestmark = pytest.mark.regression`.
3. Use the `lfd_runtime` and `api_client` fixtures from `conftest.py` — a
   fresh `lfd` per test keeps state isolation tight at the cost of ~10s of
   startup per test.
4. Link the bug: leading docstring names the fix commit or PR so future
   readers know *why* this specific shape matters.

## Weekly auto-release

When the regression suite is green on Sundays, the
[`weekly-release.yml`](../../.github/workflows/weekly-release.yml) workflow
bumps the patch version, appends a stub to `RELEASE_NOTES.md`, and commits.
`auto-tag.yml` picks up that commit and cuts the tag.

A failing regression test blocks the auto-release — no partial or unverified
cuts.
