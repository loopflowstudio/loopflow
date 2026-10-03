# Resident workspace demo readiness

2026-10-02. Jack requested recovery through demo readiness. No demo feedback or
acceptance has been recorded yet.

Jack clarified during review that scratch cleanup belongs to PR landing, not Task
delivery. The documentation had incorrectly described the resident exception as
a Task/non-Task distinction. The implementation already keys the exception on
resident workspace status; documentation now says PR landing clears scratch in
non-resident workspaces regardless of Task association. This feedback does not
complete the demo or approve merge.

Jack started the interactive demo. The feature CLI created
`/Users/jack/src/loopflow.resident-demo-1412` through `lf wt create
resident-demo-1412 --resident`, with isolated Home `/tmp/lf-resident-demo.i4ZlP4`.
The setup contains `DEMO-MEMORY.md`, `DEMO-UNRELATED.md`, and local
`scratch/resident-demo-plan.md`. `scratch/demo-env.sh` defines `lfdemo` for the
feature binary. No demo document commit, push, or PR has occurred yet. The next
interactive action is a selected-document commit and inspection of its tree.

PR: https://github.com/loopflowstudio/loopflow/pull/1412

The saved pursue Flow is waiting at its authored demo boundary, Session
`session_63fc75b803664b3db1bacc9c6bb0ad23`. Implementation and publication completed;
auto-merge remains off. The public PR excludes scratch while local notes survive.

The installed 0.12.31 runtime repairs Flow decisions. The feature itself remains
on the PR branch. Its built CLI needs an isolated Home for the walkthrough;
the production installation does not yet contain resident workspace behavior.

The walkthrough covers a Wave/repository conversation retaining its workspace
across replacement, selected document publication without a synthetic Task, and
scratch surviving publication and subsequent explicit maintenance. Scheduled
memory distribution remains outside this change.

Evidence: [design](wave-and-repo-worktrees.md), [focused checks](checks.md),
[runtime recovery](flow-runtime-schema.md), and the PR's test summary. The focused
checks and feature build passed; the installation-only Session case remains with
Linux CI. The configured-provider decision and feature PR publication were replayed
successfully. The full interactive resident-conversation walkthrough remains for
Jack's demo, not an assumed pass.

Jack requested restoring automatic command shortcuts and all short doc examples.
PR/worktree commands retain root ownership. The earlier decision to require literal
paths is superseded; review remains open while this change is implemented.
