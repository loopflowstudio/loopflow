# Release subwave demo · 2026-09-30

Jack Heart approved the LOO-354 demo on 2026-09-30 after reviewing the walkthrough
and revised memory behavior, and explicitly requested Session completion.

## Experience under review

Selecting `infrastructure/release` supplies Infrastructure's top-level Wave
documents followed by Release's, from the executing checkout. Release owns its
objective, memory and authored schedule. The [accepted design](define-durable-subwave-identity-and.md)
owns scope; [read-slice evidence](subwave-read-slice.md) and the design's gate
record retain earlier checks and their limits.

## Observed preview

At checkout commit `9385c811b`, the existing compiled `lf-prompt` rendered the
current checkout for `--wave infrastructure/release --surface cli`. The command
used a disposable Home/database with inherited LF_/LOOPFLOW_ authority removed.
The rendered Wave documents were exactly, in this order:

1. `wave/infrastructure/GOAL.md`
2. `wave/infrastructure/MEMORY.md`
3. `wave/infrastructure/release/GOAL.md`
4. `wave/infrastructure/release/MEMORY.md`

Each occurred once. No other Wave file tags appeared. The complete preview is
[release-demo-prompt.md](../.lf/tmp/subwaves/release-demo-prompt.md), a local
artifact. This is the prompt renderer's output, not a provider response or
Desktop acceptance. The earlier authored-content proof used copied files;
this preview reads the supplied worktree itself.

Preview check: compiled `lf-prompt --repo <this checkout> --wave infrastructure/release --surface cli` → four expected Wave files in order, once each; disposable data, 32.3 GiB free, no rebuild.

## Feedback and approval

Jack Heart accepted the rename/reparent behavior: Tasks keep their Wave owner
while inherited context follows the Wave's directory. Jack requested behavior-first
PR walkthroughs with examples and source snippets, distinguishing new behavior
from existing infrastructure; the builtin pr-review skill and walkthrough now do so.

Jack requested 8,000 tokens for the selected Wave's memory plus a separate
8,000-token pool for inherited memory. Each pool retains the 64-KiB byte limit.
Jack was open to experimenting with repository-wide memory. The chosen experiment
includes repository-root MEMORY.md first, even without a Wave, and shares the
inherited pool with ancestor Wave memory. Source snapshots and the overall
64,000-token / 512-KiB launch cap remain unchanged.

Jack then wrote: “ok. if that's all, then the demo is approved. complet the
session for this task”. This is explicit review approval; the configured-provider
and Desktop demonstrations remain unperformed. Approval does not manufacture
that evidence or claim installed acceptance.

Verification: isolated `cargo test -p loopflow --lib engine::exec::tests`, `cargo test -p loopflow --test context_tests`, and the native Wave seed regression → 41 passed; `cargo clippy --all-targets -- -D warnings`, formatting and diff checks passed. Log: `.lf/tmp/subwaves/budget-review-final.log`. The 49.1-GiB preflight exceeded the 32-GiB reserve. These focused checks cover the local review edits; they do not replace final-tree gate/CI.

The [working design](define-durable-subwave-identity-and.md) and
[decisions](questions.md) include the revised policy. The [walkthrough](pr-review.html)
distinguishes published source from local review edits. Complete the review with
this feedback; the following Flow decision owns navigation and remaining delivery.
After landing, installed lf owns Release's Initiative, plan/Task/KR moves and live
schedule cutover. None of those operations ran during this review.

## Completion attempt

The exact Task demo Session
`task_fd96f58071994088b3a208543fe2d015:7c115e6f-db56-4e0e-bdec-bceb2c99270e:ship-demo:review_demo:0`
refused `lf session complete`: “The session agent has not marked this ready”.
The approval was supplied in independent pr-review Session
`run_28ccbcaa69e744329cb3530eaca6a86e`; the original demo Session has no readiness
summary. Jack then explicitly requested that readiness be completed on its behalf.
The existing demo Session was resumed through lf; its agent read these notes and
published the readiness summary with Jack’s approval, both memory budgets, the
repository-memory experiment, 41 passing checks and the remaining evidence limits.
No new Session was created and no authority fields were fabricated.
