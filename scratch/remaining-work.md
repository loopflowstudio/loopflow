# LOO-298: remaining work

Jack Heart · 2026-09-30. Work in this checkout; one code writer. No branch
binary may write the installed Home. Source checks use disposable Homes with
inherited LF_/LOOPFLOW_ authority removed.

## Final model

- Exec owns one actual lf process, its causal parent and command result.
  Each executed Flow step is an ordinary child lf command. Agent-issued
  commands resolve their parent through the current provider generation.
- AgentSession owns one resumable conversation, name, feedback and native
  identity. Captures, provider outcomes, retries and usage are Session events.
- FlowSession owns one started Flow: compiled graph, cursor, return counters,
  selected completion, claim and review. Subflows and loop passes are lenses.
  Task and taskless execution use the same driver. A Task selects one managed
  Flow while permitting other attributed Flows.
- Typed decision output supplies advance, iterate or blocked with a reason.
  Blocked opens a keyed Ask and returns feedback to the same conversation.
- Task attribution is command --as → checkout → ancestor's explicit LF_AS.
  Causal ancestry grants neither attribution nor control. Bind is write-once,
  permits done Tasks and affects subsequent usage only. Work reservation or
  first bind sets Started once; inspection does not.
- A Chapter is the shared name of each Wave's one In Progress Linear Project.
  Project owns its Flow default. Rotation preserves started unfinished Tasks
  and retires only proven untouched backlog. Missing evidence stays unknown.

The implementation is in place. Current contracts belong in
[architecture-reference](../docs/architecture-reference.md) and
[CLI reference](../docs/lf-reference.md); do not reconstruct another design
from the old scratch history.

## Next work

1. Reconcile docs, builtin skills and generated pages with final behavior.
   Keep the short CLI guide/reference split. Resolve the naming proposals in
   [naming.md](naming.md) only if Jack selects them.
2. Measure cold/warm CLI list and detail on a representative dense store,
   separating startup, SQL and payload costs.
3. Prove cancellation while an exactly owned child is live. Retain unresolved
   completion rather than inferring authority from the Exec tree.
4. Run final affected suites, migration and architecture checks, build, fmt
   and all-target Clippy. Then follow the saved Flow through publication,
   Jack's demo, queue preparation and landing. This compression pass is
   authorized only to commit, push without force and stop.

## Acceptance and conversion boundaries

Configured Codex/Claude/OpenCode accounts, interactive reconnect and rendered
Desktop continuity remain unproven. Synthetic provider tests establish only
local behavior. Chapter tests cover interrupted rotation and second-Home sync;
no live Linear rotation or distributed transaction is claimed.

Jack removed historical compatibility and intermediate-draft obligations.
Retain current Waves, Projects, Tasks, PR/Linear/worktree links, accounts/routes
and resumable conversations. Three direct migrations create the final schema;
released migration history stays immutable. Keep the populated current-review
and Project-default upgrade tests: those represent existing machine state.

Before actual conversion, obtain separate authority, quiesce old writers, take
an SQLite backup and repeat the private-copy rehearsal. The existing private
converter is `.lf/tmp/deep-compress/convert_current.py`; inspect it against the
final schema before reuse. Verify selected captures, native identities, pending
reviews, Task links, account routes and foreign keys. Do not copy old turns,
process receipts or driver authority. A consistent database backup plus live
filesystem reads is not an atomic snapshot. Promotion and installed acceptance
remain outside this pass.

## Test compression boundary

Keep one maintained proof per final behavior: public Session lifecycle and
binding, headless discovery, Task/Flow retry and review, typed decisions, Exec
ancestry and interruption, and two-Home Chapter convergence. Native Codex tests
retain driver handoff, passive-client fencing, driver-loss recovery, blocked
feedback and structured results against local synthetic Responses. Retired
storage inventories, duplicate reader matrices, removed decision-command trials
and the one-off Chapter rehearsal are deleted. No configured acceptance follows.
