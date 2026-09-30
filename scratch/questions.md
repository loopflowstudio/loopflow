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

## Still open

- Confinement currently applies to unattended execution in a Task checkout,
  shared by direct skills and Flows. Jack has not selected the wider
  checkout-only or unattended-only policy; preserve the current intersection.
- A Wave with no In Progress Project has no selected automatic creation policy.
  Report the missing plan. Whole-Flow binding is outside Session bind scope.
- `session_events` → `agent_events` remains a tentative naming proposal.
  [Naming](naming.md) records the current table names.
- Configured provider/Desktop acceptance and actual quiescent conversion need
  their own proof and authority. Fixture success does not close either.

Current compression authorization: delete redundant tests and scratch, run build,
fmt, all-target Clippy with warnings denied and remaining affected tests, commit,
then `git push` without force and stop. No new product decision is needed here.
