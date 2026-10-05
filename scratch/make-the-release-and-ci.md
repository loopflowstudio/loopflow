# PR #1435 CI repair

Jack Heart authorized repairing and arming this PR, without a release or Task
completion from this active Exec. Failed head: 6c760f285; CI 37265892587.

The release cleanup assertion reproduced with a repair launcher retaining its
inherited descriptors for two seconds after the real repair Exec exited. The
checkout remained owned by release CI repair. Cleanup and repair re-entry now
wait at most five seconds to acquire an independent lease; timeout never grants
removal authority. The original checkout-count and exact-tag assertions remain.
Removed the obsolete tmux fixture, which the detached repair path never used.

Remaining: publish and arm.
After authoritative merge, a separate release/install must carry the closure
repair; stopped Flow retirement and Task completion must run outside LOO-326.

Review: independent lock acquisition remains the only cleanup authority; timeout
preserves ownership. No mutation retry, check rerun, or manufactured commit.

Checks: delayed-launcher reproduction failed as expected; broad release_run_ run 18 passed/1 failed before re-entry fix; all four final focused recovery/ownership tests pass; fmt/diff checks pass; all-target Clippy passes.
