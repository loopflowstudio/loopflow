# Preserve release ownership through staging, commits and branch pushes

## Remaining work

Keep the serial PR and Task open. Task merge revocation/compensation, notes
agents, manifest lockfile tools and source/worktree creation, reset and removal
children still need explicit capability propagation and survival proof.
Historical telemetry prerequisites and bounded current retry, dated repair
ownership, closed-obligation continuation and configured acceptance remain open.
The observed missing `agent_turns` scorecard table remains a verification blocker;
no Intelligence handoff was accepted here. Required UI-host/public smoke and two
adjacent automatic settlements, including publication without manual repair,
remain mandatory. This slice adds no new independent publication blocker or Task.

## Counterexample and implementation

Starting head: `6860d94e3bffe38428f1a3de6c57139fcb2554fa`.
The new built-CLI regression reproduced a second release successfully tagging
`v0.9.2` while the original release's staging child survived controller death.
The fixture also attempted checkout removal, but the first failed assertion was
the competing tag; the red result alone does not prove checkout preservation.

Release preparation now supplies its existing target lock and checkout lease to
`commit_workflow`. The callback reaches `stage_all`, `commit`, upstream-establishing
push and both ordinary and force-with-lease pushes. It also reaches the clean
worktree push path. Both configured and fallback commit identities use that same
inheritance. Ordinary callers explicitly supply no release capability.

Private Git output helpers share command construction and error conversion;
there is one implementation of each Git operation. No new owner, lock type,
persisted record, execution service or ambient capability lookup was added.
The callback is named `inherit_git` because it does not cover commit-message
agents, optional draft PR creation or Task merge revocation. Release preparation
uses an explicit message and disables draft creation. Its Task settlement fence
remains before pushing, with the separate revocation inheritance gap retained.

## Focused proof

The built CLI runs ten cases: staging, committing, establishing an upstream,
pushing to an existing upstream and force-with-lease fallback, each with a killed
controller or failed launcher leaving a descendant. Each observes a competing
release defer, ordinary checkout removal fail, accessible release notes, and
restored target/removal access after child exit. Failed-controller cleanup also
preserves the checkout. Staging and commit cases inspect the actual Git index
or committed release notes; push cases compare the disposable bare-origin branch
with the checkout's exact HEAD.

Commit cases execute real Git paused in a real pre-commit hook; a post-commit
hook marks the completed commit. Other cases use a shell command barrier before
executing real Git. Git repositories, bare origins, processes and OS locks are
real. GitHub and notes generation are simulated. These cases do not prove
arbitrary providers retaining descriptors, every caller-byte interruption,
installed scheduling, hosted checks, UI or public artifact availability.

Two fixture corrections were necessary after the production repair: release
branches already track origin, so upstream-establishment explicitly removes that
tracking; the branch-push barrier must leave the later independent tag probe
available. Neither correction changed production behavior. The force case
simulates an initial push failure before executing a real force-with-lease push.

## Review and validation

The simulated review followed both commit identity branches, clean/dirty workflow
paths and both push paths through the one command implementation. It confirmed
release preparation carries both held locks, while ordinary CLI/Flow/PM/Task/PR
callers supply no release capability. Existing Task revocation still precedes
head publication; the callback does not imply surviving-child protection for
that separate command. No extra draft creation path was restored. The broader
source/worktree, notes and tool children remain explicitly outside this proof.

- `cargo test -p loopflow --test release_lock_tests surviving_release_git_mutation_retains_target_and_checkout -- --nocapture`: reproduced competing tag creation before repair; final ten-case proof passed in 43.90 seconds.
- `cargo test -p loopflow --test commit_tests commit_with_push -- --nocapture`: passed, 0.31 seconds.
- `cargo test -p loopflow --test pr_tests pushed_task_commit_revokes_auto_before_exposing_the_new_head -- --nocapture`: passed, 2.69 seconds.
- `cargo test -p loopflow --test release_tests release_run_reintegrates_a_dirty_existing_pr -- --nocapture`: passed, 3.54 seconds.
- `cargo test -p loopflow --lib commit_supplies_fallback_identity_when_none_configured -- --nocapture`: passed, 0.08 seconds.
- `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings`: passed.

The final survival proof and static checks used the final executable/test content.
No affected-suite/full-CI gate, install/sync, cron trigger, production publication,
PM assignment, PR publication, landing or Task completion occurred.
