# Task checkout adaptability

Jack Heart requested fixing current workspace naming and tolerating worktrees
deleted outside Loopflow. Repair Git-metadata failures only when current APIs
can create the failing state. Historical Wave-directory migration and a review
of builtin Flows/Workflows are excluded; provider switching is handled elsewhere.

Reproduce first-start placement with an explicit agent, ordinary workspace names,
and reconciliation after deleting a checkout in a disposable repository. Reuse
the shared worktree-name normalization. Missing checkout evidence may be skipped;
retain Task, PR and landing intent without inventing completion or recreating
work during reconciliation. Other filesystem and provider errors remain errors.

The historical automatic-name error predates #1550's Task-ID-based placement.
Current explicit names still use an additional 2–5-word restriction. Current
placement selects an explicit agent before restoring/creating its Git checkout.
Reconciliation takes checkout-local Git locks even when the checkout is absent.

Fresh CLI `task create` then scoped `task run ... -a codex` reproduced the exact
missing Git-metadata error before any provider launch. The selected agent was
validated against the future path before checkout restoration. Reordered those
operations; no historical migration is involved.

Both landing observation and repository Task reconciliation reproduced missing-
path failures after ordinary directory deletion. Missing checkouts now leave
their saved requests/state intact and are skipped. Available sibling Tasks still
get observed. Removed the Task-only name parser in favor of WorktreeSegment.

Checks: `cargo build -p loopflow --bin lf`; focused library nextest (24 passed),
global_commands (38 passed, 2 disposable-installation checks deferred), integration
Task cache/initialization nextest (8 passed); clippy all-targets and fmt passed.
Tests ran headless inside the network boundary with isolated LF environment.
Red logs: `/tmp/lf-checkout-{repro,library-repro,reconcile-repro}.log`; green logs:
`/tmp/lf-checkout-verification{,-retry}.log`. The first verification caught fixture
comparisons against subsecond timestamps discarded by SQLite; compare persisted
state before/after instead. No timestamp behavior was changed.

Review: missing paths preserve saved intent; other filesystem errors still
propagate. Checkout restoration precedes provider configuration reads. The name
repair removes a duplicate policy. Disposable CLI reproduction now succeeds;
the installed Fantasia workflow has not been replayed against a new release.

## Reusing a delivered checkout

Jack Heart also requested checking reuse after an earlier PR, with or without a
Task. This checkout has no Task (`lf task status --json`). Its earlier PR #1552
merged; these follow-up changes continue here. `create_or_update_pr` finds only
open PRs, so another publication creates a new PR rather than modifying #1552.

Jack Heart chose resilience over enforcing a preferred workflow: recommend a
follow-up Task, but accept sequential PRs in one Task and show the current PR in
the main UI. No new configuration, flags or command sequence. Earlier PRs remain
history. Publishing continued work must adapt both ordinary and Task checkouts.

Implementation: allow atomically archiving a settled current PR when inserting
its successor; retain one current PR. Refresh remote settlement before publication
when the known PR is no longer open. Freeze settled placement/history, discard
old merge/completion intent for the new delivery, and keep retries on the same
new publication. Reuse merged-history preparation for ordinary publication while
preserving staged, unstaged and committed follow-up work. Follow-up Task creation
remains advice, never an implicit operation or admission requirement.

Acceptance: publish twice from the same checkout after squash merge, both with
and without a Task; repeat the second publication; retain the first PR unchanged;
recognize a merge without requiring reconcile; preserve local edits and unrelated
remote commits; do not let an earlier landing complete the latest delivery.

Review found three additional boundaries: submit still required a create-PR flag;
landing mistook leftover scratch notes for another delivery; and follow-up commits
may already have been pushed. Removed the create-PR requirement (the existing
public option remains accepted), ignore scratch-only edits when reporting an
already-merged Task PR, and preserve pushed follow-up ancestry while adopting the
replayed tree. Only an exact remote head already delivered to main can be replaced,
under an explicit Git lease; a concurrent push remains intact. Historical PR rows
and placement remain frozen, and a failed successor insert rolls back archival
and reopening. Earlier landing observations cannot complete the new delivery.

Proof uses real Git repositories and the actual publish/submit/land CLI with a
simulated GitHub boundary. It is not an installed Fantasia release replay or a
live GitHub merge lifecycle. Hosted full-matrix checks remain deferred while this
review checkpoint retains scratch, per TESTING.md.

Checks: isolated `cargo nextest` delivery/migration/docs suites (111 passed),
startup/release-lock/scheduled-release suites (28 passed), `release_tests`
(70 passed); `cargo build -p loopflow --bin lf`, all-target Clippy with warnings
denied, and `cargo fmt --all -- --check` passed. Logs:
`/tmp/lf-continuation-{reviewed-checks,shared-git-checks,release-checks}.log`.
The final merged-only landing fast path is checked separately in
`/tmp/lf-continuation-land-fastpath.log`.
