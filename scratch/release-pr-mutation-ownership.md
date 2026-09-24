# Preserve release ownership through PR creation and metadata mutations

## Remaining work

Keep this serial PR and Task open. Shared Git/commit/push, notes agents,
lockfile tools, source/worktree creation/reset/removal and Task compensation
paths still need complete surviving-child exclusion. Historical telemetry due
associations, the approved bounded current retry, dated repair ownership and
closed-obligation continuation remain unimplemented. The observed missing
`agent_turns` scorecard table and unaccepted Intelligence handoff remain the
verification dependency. Required UI-host/public smoke and two adjacent automatic
settlements, with at least one publication and no manual repair, remain mandatory.
No independent publication blocker or sibling Task is introduced by this cut.

## Counterexample and change

Starting head: `4c7a970564d48415572444887818a3beaefb13e0`.
The new built-CLI regression reproduced a second release tagging `v0.9.2` while
an earlier release PR-creation child survived its killed controller. Checkout
removal was also attempted at that boundary. The first failing assertion was
the competing tag, so the red result alone is not a checkout-preservation proof.

Release preparation previously asked `commit_workflow` to create a best-effort
draft, then required that PR in shared finalization. It now commits/pushes without
that draft side effect and asks the existing finalization owner to create the PR
with the release title and notes. An existing PR is reused. Creation failure
propagates through finalization instead of being logged before a later missing-PR
failure. No release-specific PR writer or second creation helper was added.
The ordinary commit command's draft behavior remains available to its callers.

The existing callback is now named `inherit_pr` and reaches creation, base
retargeting, title/body editing and readiness, alongside auto-merge enable/disable.
Release preparation supplies its held target lock and checkout lease. Ordinary
callers supply no release capability. The callback only configures the existing
commands; acquisition, cleanup, settlement and persisted state are unchanged.
Git integration and Task compensation paths are not implicitly covered.

New PRs use the existing non-draft creation behavior of shared finalization.
The subsequent ready command remains useful for recovered drafts. GitHub CLI's
[ready implementation](https://github.com/cli/cli/blob/trunk/pkg/cmd/pr/ready/ready.go)
returns success for an already-ready open PR. This source check is not a hosted
PR experiment.

## Focused proof

`surviving_release_pr_mutation_retains_target_and_checkout` crosses four
commands (creation, base retargeting, title editing and readiness) with two
controller exits (killed controller and failed launcher leaving a descendant).
It starts the built `lf release run`, pauses at the selected child, then observes
that a second release defers and ordinary checkout removal fails. A failed
controller's cleanup also retains the checkout. After the child finishes, the
fixture checks its simulated remote head/base/title/readiness state, restored
target access and successful checkout removal. Release-note bytes remain usable.

Git, bare origins, process boundaries and OS locks are real. GitHub and notes
are simulated; retargeting represents an existing remote PR becoming visible
before finalization. These cases do not prove hosted PR behavior, arbitrary
providers forwarding descriptors, every caller interruption, installed scheduling,
UI-host verification or publication. They cannot count toward configured KR
settlements.

The prior auto-merge replacement fixture initially timed out because it relied
on commit's draft creation to make the PR visible before initial revocation.
It now explicitly supplies the existing remote PR during preparation for the
replacement case. This preserves replacement coverage after moving creation;
no production fallback was added to satisfy the fixture.

## Review and validation

The simulated review followed all creation/metadata calls reachable from release
finalization. They use the same borrowed inheritance as auto-merge. Moving
creation removed the release call into `ensure_draft_pr`, while retaining the
ordinary command's behavior. Title/body edits and readiness still have ordinary
publication callers with separate functions; this cut does not claim a general
PR consolidation. Target exclusion and checkout removal protection retain their
separate scopes. No new DTO, store, lock owner, scheduler or settlement writer.

- New eight-case CLI survival proof: failed before repair; passed after repair
  in 60.53 seconds (22.38 seconds compilation).
- Normal release reintegration with advancing main: passed, 5.51 seconds.
- Ordinary PR arming: passed, 11.32 seconds.
- Updated eight-case auto-merge survival proof: passed, 46.83 seconds.
- `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings`: passed.

No executable change followed the new PR mutation proof. The only later test
change made the auto-merge fixture explicitly supply its pre-existing remote PR;
that fixture was rerun. Remaining shared mutators and acceptance are still open.

No affected-suite/full-CI gate, install/sync, cron trigger, production release,
PM assignment, PR publication, landing or Task completion occurred.
