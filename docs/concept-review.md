# Concept review

```bash
lf concept-review    # step back together and reconsider the current model
```

Use concept-review when the implementation is making the problem harder to
explain. Bring one concrete interaction: what the user does, what should happen,
and how they recover when it fails. Draft the affected usage docs and skill
guidance first, then use that draft to discover the types and APIs the experience
needs. Keep already-clear usage unchanged. Mark proposed behavior and preserve
the accepted requirements while exploring a different design.

With a human, work through the consequential product choice together before
designing its implementation. After review-slice, make one bounded judgment
within the accepted scope and carry forward its unresolved findings. Optional
polish need not force another pass; a required product decision must stay visible.
A useful result can be a clearer interaction, even with no infrastructure
deletion, or a reason to keep the current model.

The build, slice, task-gate, and pursue Flows place concept-review after
review-slice. Pursue then runs `loop-decide`: reviews produce evidence, and the
decision step compares the previous direction with the pass's results. A concept
proposal cannot satisfy missing behavior proof, and changing the model invalidates
earlier proof it affects. Existing pinned Flows keep their original definitions.

A pass without meaningful progress reports Blocked and opens an Ask running
`unblock`. That skill uses concept-review with the human by default, or resolves
a specific missing input directly. Human Complete returns the summary to
loop-decide for reassessment; it does not approve a separate Flow gate.

The accepted revision separates that decision into the `decide` skill after
both reviews. Reviews produce evidence; decide compares the previous direction
with the pass's results and chooses Advance or Iterate. A pass that makes no
meaningful progress reports Blocked and opens an Ask. That Ask's Session runs
`unblock`, which uses concept-review with the human by default, or resolves a
specific missing input directly. Human completion returns the summary to decide
for reassessment; it does not approve a separate Flow gate. Connecting this
revised sequence and automatic Ask handoff remains runtime implementation work.

## What our history teaches

The initial 2026-09-25 survey ranked first-parent commits on the local
`origin/main` history since 2025-01-01 by net source deletion, then inspected
candidate PR descriptions and affected paths. Source ranking excluded non-source assets and obvious test
directories; it is a search aid, not a quality score. The counts below are the
merged commit totals, rechecked with `git show --shortstat`, including tests and
documentation rather than source-only counts or estimates in older PR prose.

| Change | Simpler model | Infrastructure removed | Net lines removed |
|---|---|---|---:|
| [#872](https://github.com/loopflowstudio/loopflow/pull/872), `309575f8e` | Wave, Project, and Task became the runtime's planning nouns | Separate daemon/remote-exec surfaces, queue and fork machinery, client mirrors, app caches | 40,201 |
| [#1237](https://github.com/loopflowstudio/loopflow/pull/1237), `5f7f66833` | A provider launch leaves a manifest, events, and one terminal receipt; Work retains planning authority | SQL Invocation/Turn/Epoch/Basis execution lifecycle, replay indexes, reconciliation layers | 39,931 |
| [#1099](https://github.com/loopflowstudio/loopflow/pull/1099), `a7044e2b5` | One Run executor serves stable Work | Parallel execution-session tables, statuses, leases, CRUD, and recovery bridges for Projects and Tasks | 14,779 |

The useful chain is clearer product meaning → simpler representation → simpler
infrastructure. The product improvement is valuable on its own; following it
through captures additional gains rather than justifying the initial change.
These were substantial model changes, not simply shorter implementations of the
same abstractions. Their vocabulary is historical: later changes replaced parts
of these models again. The PR descriptions record their intended guarantees and
proof; this survey does not establish that every change remained correct later.

Selected patches substantiate the mechanism, with narrower claims than a live
preservation proof:

- **#872:** `lib.rs` removes the `lfd`, `lfdb`, and `lfq` modules; `ops/mod.rs`
  removes the branches, combine, next, and queue APIs. These are reachable
  surface changes, not just deleted private helpers.
- **#1099:** `0.11.036_delete_sessions.sql` copies Project/Task facts and rekeys
  Run and PR associations before dropping the parallel Session tables. Removing
  an identity requires accounting for its surviving references and history.
- **#1237:** `run_record.rs` records launch inputs and separates subject
  attribution from process ownership. Its terminal writer preserves an existing
  terminal receipt. Fewer concepts still need explicit evidence and authority.

The largest reductions are not automatically the best examples. Removing the
old Python runtime in #276 also deleted tens of thousands of lines, but a
language migration alone does not demonstrate a simpler product model.

## Preserve the experience

[#1270](https://github.com/loopflowstudio/loopflow/pull/1270), `e0849bad4`, replaced
resident controllers and first/loop/finally phases with a pinned Flow and finite
workers (1,498 net lines removed). The PR explicitly made Flow completion
stop without selecting another Flow; its `docs/lf.md` patch removes the documented
default feature cycle. LOO-295 subsequently recorded the user's missing
approval-to-implementation continuation experience; it is not proof of a shipped
repair. That is a concrete reason to review the whole user path after removing
a lifecycle, not just the new mechanism's local correctness.

For any proposed reduction, name the behavior that survives and trace its
normal, interruption, and recovery paths. A replacement is incomplete while old
authorities remain reachable, but deleting them is also insufficient if users
lose a required operation. A useful review can conclude that the present model
is already small enough.

## Keep the vocabulary concrete

A Flow is an authored sequence. A loop repeats part of it; a pass or iteration
traverses that part once. A slice is a bounded unit of work and may be one stage
inside a loop. A review judges behavior or design. These need not be separate
runtime entities: names should explain the experience before introducing state.

The accepted branch direction calls a Flow with one or more backward edges a
**loopflow**, accepted anywhere a Flow is accepted. That is a product contract,
not evidence that every execution path supports it yet. Task binding supplies
context and authority; it should not supply the meaning of repetition. The skill
uses the decision protocol supplied for its occurrence and does not invent a
separate runner, require a Task, or choose a successor Flow. Runtime convergence
and its proof belong to the implementation work.
