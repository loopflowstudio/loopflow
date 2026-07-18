# Open questions — PRD-38

## 1. Done-when #6's line ceiling is not reproducible, and not reachable by this cut

**Status: proceeding on a substitute criterion. Needs a human ruling before land.**

Done-when #6 requires "total repository source at or below the documented
121,819-line ceiling." Two problems.

**It is not reproducible.** No script in the repo computes it. The number enters
the tree in `docs/architecture.md:860` in PR #1073 itself. Measured at
`ae1344a57`:

| Scope | Lines |
| --- | --- |
| `rust/loopflow/src` physical | 144,210 |
| `rust/loopflow/src` minus blank/comment | 123,770 |
| `rust/loopflow/src` minus `#[cfg(test)]` modules | 95,450 |
| all Rust incl. tests | 156,461 |
| all Rust + Swift + Python | 180,504 |

None is 121,818 or 119,126. The doc's own next sentence concedes the working
tree went above the interim count, so the figure describes a mid-branch
measurement of an unstated scope, not a merged state.

**It is not reachable.** The full deletable surface, measured:

| Bucket | Lines |
| --- | --- |
| wholly deletable controller modules | ~11,260 |
| realistic net trim from contested files | ~4,000 |
| new shared-execution code added back | ~ -2,500 |
| **projected net** | **~131,500** |

That leaves a ~9,700-line gap to 121,819. Closing it would require deleting
working domain logic (`ops/task.rs` PR/CI/workspace behavior) that this task
explicitly preserves.

**Proceeding on:** commit `scripts/measure_source.py` defining the metric
(physical lines under `rust/loopflow/src`), pin the baseline at `ae1344a57` =
144,210, and gate on **net reduction ≥ 10,000 lines with deletion, not shims,
accounting for it**. The 121,819 aspiration stays a wave-level target, tracked
in `docs/architecture.md`, not this PR's gate. Flagging rather than silently
redefining an acceptance criterion.

## 2. Deleting the six legacy env vars removes the fail-closed sentinel

Resolved in the design, but worth naming as a trap. `ops/child.rs:369-385`
currently treats the presence of any of the six legacy Session vars as proof
that "this process is inside a Run", and only then refuses to fall back to User
authority when `LF_RUN_LEASE` is absent. Delete the six naively and an in-Run
process missing its lease silently becomes **User authority** — a privilege
escalation hidden inside a deletion instruction.

Design answer: `LF_RUN_CONTEXT=agent` becomes the sole positive in-Run marker,
set unconditionally at every Launch. See the design's Authority section.

## 3. Draft migration means the Session tables survive on this branch

Confirmed: `rust/loopflow/src/store/migrations/drafts/` is empty and Rust holds
no reference to it — drafts are neither compiled nor applied. So done-when #1's
"zero tables" can only mean **zero code references** on this branch; the
physical `DROP TABLE` executes at the next release canonicalization.

Fortunately `runs.lease_generation` and `runs.source_id` are both **nullable**
and their unique index is **partial** (`WHERE source_id IS NOT NULL AND
lease_generation IS NOT NULL`). The branch can simply stop writing them; the
index goes inert and no CHECK is violated. No compatibility shim is needed.

## 4. `_next` suffix means two opposite things

`project_sessions_next` / `task_sessions_next` are *legacy successor* tables
(delete). `agent_turns_next` is the *spine replacement* for `agent_turns`
(keep). Same suffix, opposite meaning. Do not pattern-match on it.
