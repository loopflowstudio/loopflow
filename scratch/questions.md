# Open questions — W2-309

Resolved in the design by executive decision; recorded here because a reviewer
could reasonably land elsewhere.

**Should the `CiIncident` row still be recorded for a scratch-clear-only head?**
Decided: yes. The head *was* red; refusing the wake is not denying the failure.
The row closes on its own (`mark_ci_incidents_green` settles by PR id, and land's
scratch clear moves the head green), and nothing re-derives an arming from a
stored incident. Consequence a reviewer should weigh: `lf ci` will show incidents
that never gain a `trigger_command_id`. That reads as "no repair was warranted",
which is true — but if it reads to anyone as "we failed to wake", the answer is a
display change, not a suppression.

**Is a unit test reading `.github/workflows/ci.yml` acceptable coupling?**
Decided: yes. It is the only mechanism that makes the `scratch-clear` literal
self-detecting rather than the drifting name list the seed warned against, and
the fact it pins is genuinely about *this* repo's land path. It breaks if the
crate is ever built from a source package without `.github/`, which is not how
this workspace ships.

**Placement of `LAND_TIME_PRECONDITION_CHECK`.** It belongs semantically beside
`ops::land::clear_scratch` — the code that resolves the check — but `task/mod.rs`
does not import `crate::ops` and `ops` depends on `task`. The const lives on the
model with doc pointers in both directions. The bind is doc-level; the workflow
pin test is what carries the weight.
