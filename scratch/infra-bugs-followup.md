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
Task. This checkout has no Task (`lf task status --json`), and its committed diff
against origin/main is empty after sync. `create_or_update_pr` uses only open PRs,
so a subsequent publication creates a new PR rather than modifying merged #1552.
No new publication was performed for this check.

Current code limits automatic merged-history restart to branches configured as
persistent (`ops/commit.rs`), although ordinary checkouts can be reused after
explicit sync. Task publication refuses a settled PR (`_task_pr_context_from_store`)
because a Task owns at most one PR. Reusing that checkout for a new Task needs an
explicit ownership transition that retains prior Task, PR and Session history.
Whether to create that follow-up Task automatically or keep creation explicit
is awaiting Jack's preference. No branch-reuse policy has been changed yet.
