# Open questions — W2-306

## 1. The adopt-time heal exceeds the literal "Done when" (assumption, flagged for review)

The seed's Done-When covers the **mint** only. I am also healing an already-recorded
incoherent base when `ensure_working_pr_with_authority` adopts an active
unpublished Working PR.

**Why.** The mint fix alone does not free W2-300 — I read the path rather than
assuming it: the active PR returns early at `ops/task.rs:3502-3508`, so nothing
re-mints the bad row, and W2-304's discard only runs inside a terminal write the
gate will never authorise. The row is permanently incoherent. The seed forbids
both remaining escapes (abandon mints a replacement — ENG-7; store surgery is
ruled out). Without the heal the reported incident stays broken and the fix is
unprovable in production.

**Why it is not scope creep.** It reuses a shipped primitive (`heal_task_pr_base`,
already called by the publish path for the opposite drift direction) and applies
the identical single authority (`merge_base(upstream, branch)`). ~10 lines.

**Why it is not "loosening Unprovable".** The tri-state and its fail-closed rule
are untouched. The heal repairs the *data* the gate reads — replacing a base the
mint fabricated with the fork point the branch actually sits on. Carried work
still reads `Range`, never `ProvenEmpty`.

**Decision:** proceed. Reviewer can cut it and lose only the live W2-300 repair;
the mint fix stands alone.

## 2. The publish-path `M < B` message misreports this defect (real, deliberately out of scope)

`ops/task.rs:1497-1511` classifies `merge_base < base` as *"PR range is
contaminated: recorded base carries commit(s) not on origin/main"* — the
`#877/#882` foreign-ancestry case.

W2-300's row hits that arm (`is_ancestor(9f6bdd498, 3e9df0677)` is true), but the
message is **wrong** for it: base `3e9df0677` *is* `origin/main`'s tip and carries
nothing foreign. The arm conflates two causes:

- (a) base is on the upstream but ahead of the branch's fork point — the mint bug;
- (b) base is off the upstream carrying foreign commits — genuine contamination.

`is_ancestor(base, upstream)` distinguishes them cleanly.

Not fixed here: W2-300's successor must be **discarded**, never published, so this
path is not where it is stuck, and the mint fix makes cause (a) unrepresentable
going forward. Worth a follow-up task for the diagnostic message. Noted rather
than filed — filing from a Task session is not this Task's job.

## 3. RESOLVED (ir_0c7865d9): "no replacement row" was unsatisfiable — assertion corrected

The reviewer asked whether the fix can deliver Done-when #1's "no replacement row
is minted", given event 10293 minted a successor while the Task was terminal.
Answered from the code; full evidence in the design doc's "Does the fix deliver
'no replacement row'?" section.

- **(a) false.** Three terminal fences exist (2172, 2187, 3499) — but all read an
  in-memory `session.status` snapshot loaded at 2165, and nothing re-reads it
  (`reconcile_task_pr_with_authority` refreshes only `observation`;
  `reconcile_task_completion` only writes). The store doesn't fence it either:
  `validate_task_pr_settlement:3982` validates PR shape only, never session
  status — unlike `complete_task_session_with_lease:585`, which *does* assert
  `status == Completed` for its `skipped_pr`.
- **(b) true.** The fix changes what the row *contains*, not whether the mint
  fires. Assertion corrected to "no **incoherent** row; completes exactly once and
  **stays** completed". The stray coherent row is provably inert: `ProvenEmpty` +
  merged predecessor → `discardable_successor` with **no blocker** (4198-4200) →
  `gate.satisfied` (4216) → `repair_premature_completion` returns at 4315 without
  reopening. **The reopen, not the mint, was the loop.**
- **(c) true and separate.** Terminal-must-dominate-rotation is a real second
  defect (stale-read fences + unfenced store write). Natural home is the store,
  which already fences the symmetric `skipped_pr` transition. Not chased; scope
  unchanged, per the reviewer's explicit instruction.

Worth filing as a follow-up Task alongside item 2. Wave memory already records the
event chain (10291-10294) as a W2-300 finding, so the evidence survives this
session — but a review finding dies with the Task it was delivered to, so it
needs a Linear task, not just memory.

## 4. Whether any other live Task carries an incoherent base

Not yet measured fleet-wide. The `Measure` section names the query. If others
exist, they are freed by the same adopt-time heal with no extra work — but the
count is worth reporting as evidence rather than presumed to be one.

## Resolved during kickoff (recorded so they are not re-litigated)

- **Directive premise** — verified exactly, all five claims (store, both refs,
  both ancestry directions, worktree checkout). No correction needed.
- **Which fix option** — the directive offered "reset the branch" or "record the
  base the branch sits at". Reset is fail-destructive (kills cherry-picked carry);
  naive "base = branch tip" is fail-open (marks real work empty). `merge_base`
  is the only option that is neither, and it is already the system's stated model.
- **Duplicate work** — #1058/#1052 touch `ops/task.rs` but not the base
  computation; #1041 is scratch-only. Checked before designing, per the standing
  rule that a green unmerged PR is invisible to every local signal.
