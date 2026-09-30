# Restart scope and incident dispositions

LOO-298 · Jack Heart · 2026-09-30. Assessed against `fd9475a5f` and the source
of landed #1359 (`00cf9dffe`) and #1363 (`458fa141d`). No installed-store writes
or configured provider mutations were performed.

## Restart scope

Jack Heart's supervising session requested a workflow check before adding an
engine-restart operation. The recorded needs already have owners:

| Need | Existing operation and boundary |
| --- | --- |
| Continue a conversation from another client | `session connect` transfers the driver and retains the live engine. |
| Replace terminal clients | `session connect --replace` stops exact owned clients, retaining the engine, active turn and sibling conversations. |
| Resume saved work after driver/engine loss | `flow resume FLOW --retry` uses recorded process/endpoint evidence and native history; confirmed engine exit permits replacement. Unknown liveness does not. |
| Replace a Task's saved workflow | `task restart` stops the selected work before replacing the Flow. It does not reset a conversation engine. |

No separate workflow requiring forced replacement of a live engine was found
in the accepted requirements. The architecture and CLI references now describe
these operations without promising another restart command. This is the
implementation assessment authorized by the latest steer, not new acceptance
from Jack or a claim that an unknown/wedged engine has been recovered.

The retained public-connect and driver/engine-loss fixtures already exercise
these boundaries with real Codex and synthetic Responses; their recorded results
remain in the working plan. No new native proof was run for this documentation
change. Configured provider and Desktop acceptance remain open.

## Incident dispositions

**Task-row/publication — retained identity, separate base blocker.** #1359's
`ops/pr.rs` attaches a known existing or acknowledged-created PR before fallible
readiness/enrichment. `ops/task.rs::attach_task_github_pr` persists that identity,
revokes stale merge intent and retains provider-link errors. The maintained
public tests exercise failed reads and readiness, retry without duplicate PRs,
and retained Task PR identity. This repairs the identified loss window; it does
not reconstruct the historical incident's exact cause. This branch's subsequent
merge-base publication refusal is still LOO-351, not lost GitHub identity.

**Stacking — an existing Task keeps its identity and history.**
`ops/task.rs::stack_existing_task` selects the retained parent PR under the
mutation lock. `store/sqlite/children.rs::stack_task_pr` owns the transaction;
an active claim prevents mutation, and retry preserves the same parent. Generic
PR updates do not write `parent_pr_id`. The focused proof checks rejected
stacking leaves claims/history intact, successful repeated selection after
release preserves the Flow/history, and self/cyclic dependencies are rejected.
Selection does not move a checkout or change GitHub's base; later integration
owns those effects. No configured stack publication was demonstrated here.

**Refused start — preflight prevents creation; later failures retain work.**
`prepare_new_task` resolves launch inputs and placement before provider creation.
The planning operation's refusal proof leaves synthetic provider inventory empty;
its lost-response retry finds the same issue and retains the provider title.
The unpublished-parent proof refuses before allocating a child checkout. This
dispositions preflight refusals without adding compensating deletion. A failure
after successful provider creation may leave the issue for recovery; this is
not a promise that every failed launch leaves no issue. Explicit Task deletion
retains the separate LOO-305 semantics and does not prove process termination.

#1363 narrows the old Task controller's permission-diagnostic scanner. That
scanner and the separate Task provider launcher are already absent here:
Task dispatch delegates to the ordinary Flow driver, which consumes typed
outcomes. Do not resurrect the scanner while integrating main. #1363 does not
prove issue preflight, cleanup or configured-provider recovery.

## Integration and verification

Both attempted merges were aborted from a clean source tree. Main at `e14a1d035`
conflicted in twelve files; the smaller merge through #1363 conflicted in
`controller/task/mod.rs` and `session_record.rs`. #1362's context-budget and steer
acknowledgement changes need translation into the Session/shared-command model;
the later account-routing change also needs integration. No upstream merge is
claimed by this iteration. Preserve merge-only delivery and finish integration
before the supervisor's final gate.

The scrubbed, disposable-Home nextest run passed all five named incident tests
above (1,998 unrelated tests skipped). Exact command/output:
`.lf/tmp/test-compress/incident-dispositions.log`. GitHub and planning responses
are synthetic; these passes establish no configured-provider operation. No
production code changed in this iteration. The integrated gate remains with
the supervising session.

Documentation checks passed: architecture coverage, both required website
documentation tests, generated architecture HTML consistency and `git diff
--check`. The renderer initially lacked `fasthtml` in the root Python project;
rerunning through the website's declared environment succeeded. Website copies
were regenerated. These are content checks, not rendered Desktop acceptance.

Review retained three distinctions that would otherwise overstate the result:
client replacement versus engine recovery, preflight refusal versus post-create
failure, and stacking selection versus Git/GitHub integration. The subsequent
compression reconciles the architecture pages with one FlowSession's cursor and
return counters, ordinary child Execs and current-state conversion. The broader
documentation audit remains on the next item's list.

Compression verification: architecture coverage, both website documentation
tests, generated HTML consistency and `git diff --check` passed. Website copies
were synced. This changes documentation only; runtime proofs were not rerun.
