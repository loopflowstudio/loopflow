Jack Heart's November 3, 2026 direction: opening a terminal workspace should return people to the right Loopflow Task Sessions, much as opening Desktop returns to familiar work. Rex is primary, Herder secondary; cmux is a possible later adapter. This supersedes this Task's earlier cmux-first trial priority.

LOO-709 removes lf resume --all, including its Desktop startup caller. A blanket restart of every interrupted Session is not the desired workspace experience. Develop a principled workspace launch/restore interaction; lf open workspace is a candidate name, not an accepted command design.

Desired outcome:

* Open a deliberately selected workspace of Loopflow Work and its existing Task Sessions in Rex. Reopening is idempotent: reuse/focus existing panes and Session identities, preserve worktrees and provider history, and never replay a turn merely to paint the workspace.
* Decide how workspace scope/membership is selected (repository, Wave, explicit retained set), and how it survives close/reopen or restart. A workspace is a presentation of existing Work, not another execution or planning owner.
* Explore automatically opening, hiding or closing Task views as Task state changes, like Desktop. Define these rules explicitly; preserve active focus and unfinished input. Closing a view never abandons/completes a Task or stops its live work.
* Restore familiar state without eagerly starting agents for every historical Session. Preserve LOO-709's drafts, answerable pending requests, live watches and no-replay guarantees.
* Validate the complete select/open -> work in one Task -> return -> close/reopen path in Rex. Scope and validate Herder second, recording actual API limitations. No equal-priority cmux implementation requirement.

Retain the earlier arrangement foundation: consume a shared Work selection/arrangement plan where supported, retain Work-derived titles and explicit attention/status, never infer waiting from inactivity, and reuse existing Sessions instead of duplicating execution. Partial host updates must be inspectable and retryable without destroying unrelated panes. Do not invent a second selection model, Lua runtime or generic multiplexer platform. Verify actual installed/public host APIs without copying competitor source.

Design decisions still open: workspace membership, persisted focus/layout, state-driven view rules, startup readiness and partial-failure UX, and final command shape. These require product design before implementation; this capture does not select a worker.

Coordination:

* LOO-709 / PR #1804 owns idle ending, interactive-state preservation and removal of bulk resume; workspace launch belongs here.
* LOO-672 owns Desktop conversation readiness/restart; preserve its still-reserved review decisions and reuse its Session semantics.
* LOO-500 / PR #1803 changes Wave lifecycle and shared workspace selection; LOO-673 / PR #1791 changes Desktop navigation/selection and abandon/unassign presentation. Coordinate shared selection APIs and preserve their work.
* LOO-497 and LOO-489 own the shared selection/arrangement and navigation foundations; inspect current delivery before depending on older proposed --plan APIs. LOO-416 owns prior Desktop lifecycle work.
* LOO-421/427 supply host trial/API research; LOO-423 retains newcomer polish, LOO-453 readiness/event details and LOO-422 explicit host status. Their historical designs are evidence, not current API proof.

Acceptance: show a real Rex workspace opening the selected Task Sessions; repeated opening creates no duplicate Session/provider; reopening returns to the intended work without replay; membership/state changes follow the agreed rules without stealing active input; partial failures preserve unrelated panes; demonstrate the agreed Herder scope and name remaining adapter limitations.

## Task workspace shells and retention ownership — November 3 follow-up

Jack Heart additionally requested opening a specific LF Task as a preconfigured Rex workspace: its relevant terminal tabs/splits grouped in one Rex session, with each shell on the Task's actual local or remote Machine and recorded checkout. Confirm the exact Rex session/window/block mapping through the supported API. Ordinary `lf ...` typed inside those shells runs on that Machine; no per-command remote wrapper is required. This adds a concrete single-Task entry case without discarding the existing wider workspace design or Rex-first/Herder-second priorities.

Coordinate with [LOO-717 · Local and remote Task shells have predictable lifetimes and recovery](https://linear.app/loopflow/issue/LOO-717). That Task owns the Desktop shell lifecycle/cleanup contract. This Task owns materializing and restoring the selected workspace in Rex. Mobile is outside this follow-up.

Before implementing retention, map Rex's terminal/PTY ownership against lf's detached execution owners. Decide whether ordinary shell retention belongs to Rex, lf, or a deliberately separated combination; assess Desktop/app-owned versus independent host-service responsibilities with LOO-593. Jack raised this as an open architectural question, not approval to relocate all execution into Desktop. Avoid two authorities independently retaining/terminating the same work, a nested multiplexer with no purpose, or duplicate agent conversations. Distinguish retaining the lf viewer from retaining its underlying execution.

Demonstrate opening the selected Task's layout, opening it again without duplicating work, leaving/reopening it under the agreed policy, and explicit cleanup. Unrelated Rex panes stay intact; remote failure never falls through to a local shell; retained processes remain discoverable and terminable. Preserve LOO-709's accepted interactive-state and active-work protections. An arbitrary shell's variables, jobs and editor state cannot be recreated merely by restoring a screen or provider history, so do not copy agent idle-ending policy blindly.
