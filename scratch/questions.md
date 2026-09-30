# LOO-298 decisions and open choices

Jack Heart · 2026-09-30. This is the live decision list; superseded proposals and
pass histories stay in Git.

## Settled

- One actual lf process is an Exec. Flow steps run ordinary skill/command
  children; no second provider executor. Definitions compile before execution.
- One started Flow is one FlowSession. Subflows and loop passes are display
  lenses, not child Sessions, claims or lifecycles.
- Captured input is a Session event. No Run, Request or replacement attempt
  object. Exec outcomes and provider outcomes remain distinct.
- Direct-command behavior governs Task steps. Task context follows the checkout;
  all agent starts use the same participant resolver. Credential failures belong
  to accounts and the shared retry/failover path.
- Attribution precedence: this command's --as, then checkout Task, then inherited
  explicit LF_AS. Task commands supply --as. Session ownership, process ancestry
  and claims never infer Work.
- Bind affects subsequent usage; earlier usage keeps its recorded owner. Do not
  invent token splits for active cumulative turns. Intelligence re-evaluation is
  a follow-up direction, not a Task filed by this pass.
- Blocked is structured output with a required reason. Its keyed Ask returns
  feedback to the same conversation; it supplies no navigation verdict.
- Child executable resolution: leading PATH lock, ordinary PATH, selected
  installation, driver fallback. LOO-334 owns the recursive lock mechanism.
- Delete historical import, old-format/selector compatibility and intermediate
  drafts. Keep current Work/links, accounts/routes and resumable Sessions.
  Migration rehearsals use copies only; no installed-Home write or promotion.
- One item per implement iteration, focused checks between items, full local
  Rust coverage before final gate. LOO-334 continues independently.
- Jack Heart authorized autonomous landing on 2026-09-30: “try to do this all
  autonomously, no need to review with me.” No demo or review wait remains.
  Land #1296 as one PR; #1358, #1359 and #1360 already landed the independent
  pieces. Exec/Chapter extraction would save only about 10% and requires manual
  cutting, so the old landing-groups proposal is superseded.
- Merge main into this branch; do not rebase. `e3a2c7e2c` merged #1360.
- Only this machine is a client. Jack reports the pinned dev Home is gone and
  its active Tasks were moved by hand to the one main Home, `~/.lf`. Do not
  recreate it or preserve history/compatibility for nonexistent clients. Three
  migrations remain; current operating state and resumable Sessions still matter.

## Still open

- Confinement currently applies to unattended execution in a Task checkout,
  shared by direct skills and Flows. Jack has not selected the wider
  checkout-only or unattended-only policy; preserve the current intersection.
- A Wave with no In Progress Project has no selected automatic creation policy.
  Report the missing plan. Whole-Flow binding is outside Session bind scope.
- `session_events` → `agent_events` and the earlier `exec_events` rename remain
  open, unapplied proposals. The latter has no surviving `run_events` table to
  rename. [Naming](naming.md) records that observation without closing the choice.
- Configured provider/Desktop acceptance and actual quiescent conversion need
  their own proof and authority. Fixture success does not close either.

Current realign authorization: reconcile the plan and Infrastructure memory,
commit, push without force and stop. Subsequent autonomous landing belongs to
the supervising session. [Remaining work](remaining-work.md) separates merge
requirements from release conversion of `~/.lf`; neither installed migration
nor promotion is part of this pass.
