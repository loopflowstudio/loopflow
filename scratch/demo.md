# LOO-691 demo: releases resume, GitHub issues hold `stable`

Reviewed 2026-11-04, headless, by the agent standing in for Jack Heart at the
`code` graph's demo step. No person watched this. Design: `scratch/design.md`.
Assumptions: `scratch/questions.md`.

Boundary (Jack Heart, 2026-11-04): read-only `lf release` commands and fake `gh`
only. `regress` and `promote` never ran against loopflowstudio/loopflow; a final
read shows no `regressed-in` label and v0.15.0 still Latest.

The 2026-11-02 steer that the demo ends with Jack reviewing the Asana experience
is superseded: parity moved to LOO-726 on 2026-11-04.

## Observed

Branch build `target/debug/lf`, disposable `LF_HOME`, inherited `LF_*` cleared.

**Real GitHub, read-only**

| Command | Result |
|---|---|
| `lf release status` | Preview v0.15.1, Stable v0.15.0, "v0.15.1 soaks until 2026-11-06" |
| `lf release issues` | "No open regressions." |
| `lf release issues v0.15.1` | "No regressions reported against v0.15.1." |
| `lf release promote --dry-run` | "Stable: v0.15.0 (stays)", soak line; nothing moved |
| `lf release check` | exit 0, 24 commits since v0.15.1 |

**Regression lifecycle, stateful fake `gh`, local bare origin**

| Action | Result |
|---|---|
| Clean, v0.9.2 soaked | "would move to v0.9.2" |
| `regress v0.9.2` twice | label created once, two issues filed |
| `issues`, `issues v0.9.2`, `issues v0.9.3` | both issues, "holds v0.9.3, v0.9.2"; v0.9.3 reads as carrying them |
| `promote` while open | "Stable: v0.9.1 (stays)", no publisher call, no `release edit` |
| Closed not planned / duplicate | hold drops; reopening restores it |
| Label removed | that issue leaves the list |
| Closed completed | v0.9.2 and v0.9.3 stay held; a release published afterwards waits on soak |
| Soaked release after the fix | `promote` runs `publisher promote --tag v0.9.5`, then `gh release edit v0.9.5 --latest` |
| Regression against stable v0.9.5, v0.9.6 soaked | stable stays, "#9 holds v0.9.6", never moves back |
| Issues unreadable | `status` exit 0 with the reason; `promote`, `promote --dry-run`, `issues` exit 1 |

**Fetch fix, negative control**

`release_run_fetches_a_release_merge_it_has_not_seen` passes. With the
`git fetch origin` removed from `ensure_commit_local` it fails: "commit d17b1b1…
is not present locally even after fetching origin". Source restored, tree clean.

## Decision

Advance. Every behavior in the design's Experience section held on the real
binary. This is the agent's call standing in for Jack Heart, not Jack Heart's
statement: the PR body already carries the proof limits below, and they need a
merged build to close.

## Proposals, not agreed

- `lf release issues v9.9.9` answers "No regressions reported against v9.9.9." for
  a release that does not exist, so a typo reads clean. Small; no change made.
- `lf release status` says nothing about a regression filed against current
  `stable` until a newer release exists. `lf release issues` shows it as
  "holds nothing". Matches the design; worth a look when it first happens.

## Unproved

- A real release resuming on the dedicated host without a manual fetch: needs this
  fix on that host through `dev`. Verify step.
- `gh label create` and `gh issue create` against real GitHub: flags match
  `gh --help`; the first `lf release regress` proves them.
- `lf` reading a real labelled issue: none exists. `state`/`stateReason` values
  rest on the design's read-only check of 2026-11-04.

## Next

Gate merged main (#1805, #1820, #1818) locally and resolved `children.rs`. PR #1780
stays CONFLICTING on GitHub until the next publish pushes that merge.
