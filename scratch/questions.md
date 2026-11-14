# Open choices (LOO-896)

- Revert PRs #1931 (#1921) and #1883 (#1881) are still open from the old bisector. Plan leaves them open: closing another PR is outward-facing and #1931 may reflect a real failure. Closing them is a one-line `gh pr close`.
- `scripts/macos_main.py` is deleted and its two remaining branches inlined into `macos-main.yml`, rather than kept as a smaller script as the Task's "Done when" phrased it. Same behavior, one less file and test.
