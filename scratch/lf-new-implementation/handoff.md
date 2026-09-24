# LOO-291 integration handoff — 2026-09-23

The human asked: “lets get this into the task flow and then advance the task.”
The existing Product / Desktop Task LOO-291 already explicitly includes lf-new's
worktree grouping and launch UX. Reuse it; do not create another planning Task.
The human also authorized installing this build to perform remaining live proof
and remove the abandoned `list` and `engbot` Wave registrations.

## Concrete contribution

`jack-heart/lf-new` at `561eadccefa6bbf1d67cd832cbe7b5df728d6c8e` includes the
committed `main-view-task` UI through `cf4f24209`, plus:
- Scope-aware New conversation (repo/Wave/Project/Task) using configured app or
  terminal, and separate ordinary New terminal.
- Outer worktree slots, inner terminal splits, retained companion shells.
- Actual PTY-backed shell/client attachment; external clients remain elsewhere.
- Shared current-Wave filtering, roadmap --all ignoring inherited Wave scope,
  and exact empty-abandoned-registration cleanup.

Full design and evidence are in that branch's scratch/lf-new.md and
scratch/implementation-evidence.md. The contribution's implementation is
committed; it is not an unimplemented design anymore.

## Updated product intent

The human reconsidered mandatory Task/worktree creation and proposed that zoom
level chooses the default conversation. This slice starts a fresh scoped human
conversation without creating a Task, forcing a destination, running an
operating pass, replacing another provider, or enforcing one-current-Session
cardinality. Preserve every existing Session. Explicit Task creation/adoption
and bounded conversation lifecycle remain open follow-ups; reconcile with the
full LOO-291 design rather than silently enforcing its earlier proposal.

## Integration and next step

Keep LOO-291's owned checkout and branch. Preserve its ongoing edits, including
its commits after cf4f24209. Integrate jack-heart/lf-new using lf's rebase workflow
inside the Task checkout after checkpointing its coherent current work. Resolve
any conflicts in favor of one unified navigator, retained Session store, and
worktree workspace registry; do not restore the former Sessions-only sidebar.
Do not mutate or adopt the source branch as a second active Task branch.

Then run the narrow proof for reconciled behavior and advance the Task's normal
flow. Preserve original directive-editing, shared Session API integration, and
external-work/performance proof obligations. Do not claim those complete from
this contribution. The provided local native tests and fixture capture do not
prove real configured provider/app launch or the user's interaction approval.

## Existing proof

Ghostty checks passed for two local shell clients versus another window, and
for an initial launch command exiting into a usable shell. Sixteen integrated
workspace/navigation/launch tests passed. Rust proofs cover PTY identity,
roadmap reads from / with inherited Wave IDs, and guarded deletion/dry-run.
Both Xcode build-for-testing and cargo clippy --all-targets -- -D warnings passed.

Installation succeeded via scripts/install.py local --use (candidate 0.12.19+561eadcce).
Current repo Wave reads contain exactly infrastructure, intelligence, product.
engbot was deleted through lf work forget. list remains historical because it
owns abandoned Project task-first-control-plane, Task LOO-276, and a PM snapshot;
it is excluded from current navigation. Preserve that history.
The app is open for human launch verification. AX automation is disabled for
this host; the user has been asked to click New conversation. No provider-launch
or user-confirmation success is claimed. Preserve this open demo boundary.
