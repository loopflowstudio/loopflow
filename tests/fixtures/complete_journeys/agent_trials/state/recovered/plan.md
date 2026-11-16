# Complete through saved state

This slice: Complete persists the Task removal, updates the Desktop and mobile headless list consumers, and selects the remaining item. A stale reply cannot restore the item, restart reads the saved change, and refusal leaves storage and both selections unchanged. Empty lists select nothing.

The source of truth is the JSON record owned by `Store.complete` in `state.py`. `TaskList.receive` already rejects older revisions and reconciles selection. Complete currently changes only local state, bypassing that owner. Route the action through the existing store and distribute its returned revision through receive, including the initiating consumer even if the supplied consumer list omits it. Refusal propagates before any consumer changes.

Delete — do not maintain: the independent permission check, local removal and selection decision in `TaskList.complete`. Retain Store permission handling, persisted tasks/revision, and receive's stale-reply and selection rules. No schema change is needed. No parallel state owner or reducer-only completion.

Alternative: optimistic local removal would require rollback and still needs store reconciliation; the synchronous saved-result path directly satisfies this fixture.

| Responsibility | Consumers | Exit check | Status |
| --- | --- | --- | --- |
| Complete persistence and list reconciliation | Desktop and mobile TaskList | saved removal, both lists and next selection | Passed |
| Stale reply and restart | receive and new TaskList | removed task stays absent | Passed |
| Refusal and empty selection | both lists and Store | no change on refusal; empty selects None | Passed |
| Create | outside fixture | full-target acceptance | Pending |
| Reopen | outside fixture | full-target acceptance | Pending |
| Native click-to-paint timing | native apps outside fixture | native measurement | Unmeasured |

Focused acceptance: `uv run python -m unittest state.CompleteJourney` exercises the headless consumers with real file persistence. These checks cannot establish native UI timing or full-target completion. Stop after this Complete journey; Create and Reopen remain pending.

Observed: baseline ran 3 tests with 2 failures; after consumer cutover, `uv run python -m unittest state.CompleteJourney` passed all 3. `uv run python -m py_compile state.py` and `git diff --check` passed. Complete is covered for this headless fixture; Create, Reopen and native timing remain outside the completed slice.
