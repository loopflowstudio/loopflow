# LOO-355: composable Task lifecycle

Jack Heart requested lifecycle composition, issue-addressed Task actions, and
cancellation of open issues left outside current chapters (2026-09-30).

## Contract

Task owns the Linear outcome, local disposition and execution; PR owns GitHub
and its retained Task PR record; worktree owns local checkout and branch plus
remote branch. Abandonment composes downward. Completion composes landing with
the existing authoritative-merge completion gate. Lower operations explicitly
report retained higher-level state. Errors remain errors; retry observes already
completed effects. Missing checkout/remote branch is a supported recovery case.

Never infer cancellation from missing membership. Protect completed issues,
current-chapter sweep members, dirty checkouts without explicit force, and live
or unknown execution. Preserve Task/PR/Run history. Sweep is dry-run by default;
report open PRs and execution as exclusions before applying safe candidates.

## Current implementation

The action/ownership inventory is in `docs/lf-reference.md` under Lifecycle action
inventory and copied into the prepared PR body. Task cancellation now uses the
same PR abandonment operation as `pr abandon`; that operation uses the same
checkout/branch deletion as `wt delete`. Missing refs and checkouts support retry.
Task repository selection follows retained Wave ownership before requiring the
caller to be in Git. PR and rebase operations are reachable by issue ID.

Sweep includes archived Projects and issues, defaults to preview, rechecks chapter
membership on apply, and reports exclusions and incomplete operations. It reads
issue attachments too: an open PR without a retained Task PR record is excluded,
not inferred absent. GitHub branch matches include the source repository, so
same-named branches in other forks are not closed. Attachment URLs cannot choose
a host for GitHub credential-bearing reads.

## Remaining work

- Resolve completion/landing checkout cleanup at Task-worker settlement; retain
  the existing completion gate and captured Flow. The cancellation implementation
  must not be treated as proof of the full requested completion/deletion symmetry.
- Reconcile Task deletion and terminal-outcome recovery with the full action
  coverage. Current differences and preservation constraints are in questions.md.
- Demonstrate abandonment on a disposable configured Task across Linear, the
  owning store, GitHub and Git. Source fixtures do not replace that live proof.
- Run the configured Wave sweep preview, then apply eligible cancellations.
  Close LOO-309 and LOO-329 through the new operation. The selected CLI does not
  implement it yet; no source access to the installed store or promotion occurred.
- Caller owns publication, installation and Flow navigation. LOO-355 is unfinished.

## Proof and review evidence (2026-09-30)

All Rust tests ran with inherited `LF_*` authority removed and `LF_BIN` pinned to
the source CLI path, against disposable stores and Git remotes. Linear and GitHub
responses were simulated; no provider credentials or production issues were used.

- `cargo test -p loopflow --lib ops::wt::tests -- --test-threads=1`: 2 passed.
  Proves both branches and checkout disappear, retry without refs/checkout,
  dirty/default-checkout preservation and remote advancement rejecting deletion.
- `cargo test -p loopflow --lib task_abandon -- --test-threads=1`: the composed
  cancellation and store-claim tests cover issue, branch and checkout selectors,
  an issue-addressed caller outside Git, provider refusal, partial GitHub failure,
  explicit lower-layer notice, retained history and cleanup retry. Final fence
  verification passed both tests, including claim rejection while cancellation
  intent is pending.
- `cargo test -p loopflow --lib task_sweep_ -- --test-threads=1`: 1 passed.
  Preview leaves issues unchanged, current/terminal issues survive, an untracked
  open PR is excluded, eligible planning work is canceled and apply retries.
- `cargo test -p loopflow --lib task_lifecycle_commands_ -- --test-threads=1`:
  1 passed. Lower PR flags retain their values and wt remove/rm are absent.
- `cargo test -p loopflow --lib task_completion_ -- --test-threads=1`: 10 passed
  before the final cancellation-only changes. These preserve existing completion
  behavior; they do not prove the unimplemented landing-cleanup symmetry.
- `cargo clippy --all-targets -- -D warnings` passed after the final worker
  intent fence. Formatting and diff checks also passed.

Earlier failed attempts exposed an invalid review fixture without a node ID,
missing current-Project fixture responses, duplicate fixture Project names and
macOS /var versus /private/var path identity. Fixtures were corrected. A final
compile used an unavailable direct url dependency; it now uses the existing
reqwest URL re-export. Those failures are not passing proof.

The simulated review changed the implementation: propagate provider/Git failures;
read back cancellation; preserve primary/dirty/advanced remote work; resolve
historical branches after cleanup; refuse claimed workers transactionally; use
existing intent to prevent a claim racing the provider request; inspect untracked
PR links; and match source repository identity. No full gate or CI ran.

Read-only configured GitHub observations: PR #1299 and PR #1318 both returned
CLOSED, with headRepository loopflowstudio/loopflow. This confirms the PR facts
only. The installed CLI remains 0.12.26 and rejects task abandon and task sweep;
it provides neither a configured sweep result nor Task cancellation evidence.
