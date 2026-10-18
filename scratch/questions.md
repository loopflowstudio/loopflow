# LOO-505 remaining implementation choices — 2026-10-17

Jack Heart approved `keep-tasks-single-homed.md` after the corrections recorded
in `direction.md`. No unanswered product decision blocks implementation.

Preserve the existing Project-derived Wave relationship. New work is explicit
repository identity, nullable Project membership, Task → Wave → repository
workflow selection, direct Wave default ownership and identity-preserving
membership moves. `task move` handles membership; `task workflow move` handles
position. Projects retain no historical workflow selections.

The migration uses the existing `waves.current_project_id` pointer; no
status-based election or historical fallback remains to decide. Live planning
and other Task checkouts remain outside migration experiments. Gate and native
demo evidence are tracked in the plan; demo review remains required before landing.
