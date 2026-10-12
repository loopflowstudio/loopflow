Agents can open and arrange Loopflow Desktop around repository, Wave and Task identity. Each Work keeps its tab and panes in one repository window across Machines.

## What changes

- Add Desktop discovery, opening, shell splits, bounded terminal reads and targeted input. Linux reports unsupported Desktop operations.
- Keep Sessions separate from views and preserve pane execution locations. Consolidate Wave planning, monitoring/history, and context/explanation flags.
- Exchange subtree delegation through the shared Work journal. New Tasks reserve one permanent execution Machine before checkout; concurrent starts and lost replies keep the same owner. Started Tasks stay put.
- Remove required post-merge follow-up and its APIs. Bare landing leaves the Task open; optional new work uses ordinary Task creation before completion. Private recovery preserves historical reserved filings and uncertain exports.

Builds on the merged [Share saved Work across Machines · #1491](https://github.com/loopflowstudio/loopflow/pull/1491). Historical shared Tasks without a coordinator still refuse; recovery remains [LOO-414](https://linear.app/loopflow/issue/LOO-414).

## Checks

Rust: 2521 cases passed across the broad run and focused reruns; 18 skipped. Integration sync interrupted tests that rebuild the CLI; all affected cases passed on the stable checkout. Desktop build and all 502 headless Swift tests, formatting and all-target Clippy passed. Architecture coverage, 21 architecture tests, portable HTML and the isolated checkout benchmark also pass. CI owns the release matrix.

Native and configured remote proof remains [LOO-391](https://linear.app/loopflow/issue/LOO-391); this PR makes no new native usability claim.

## Try it

Suggested walkthrough: run `lf --task TASK desktop open`, inspect `lf desktop list --json`, and split its pane. Switch Work and return to its retained arrangement.
