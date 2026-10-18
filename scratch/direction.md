# LOO-505 direction

2026-10-17 — Jack Heart requested including repository-level default workflows in
LOO-505. This extends the design scope for standalone Tasks.

2026-10-17 — Jack Heart corrected workflow ownership and selection:
**Task → Wave → repository**. Associate workflow defaults directly with Waves;
do not retain workflow selections on historical Projects. This supersedes the
earlier Project-default proposal. Remove Project workflow storage and consumers
as part of this Task rather than maintaining both owners.

Already captured Task workflows retain their definition and position when
defaults or membership change. Task execution history remains independent of default selection.


2026-10-17 — Jack Heart requested renaming membership `refile` to `move`.
The design uses `lf task move` for membership and relocates the existing position
operation to `lf task workflow move`. No ambiguous compatibility alias remains.

Jack Heart asked whether deriving Wave through Project was a change. Inspection
confirmed `TASK_COLUMNS` already selects `p.wave_id`; the design preserves that
relationship while adding repository identity and nullable Project membership.

Jack Heart then approved the design (“lg2m”). Implementation through the feature
workflow's demo review/publication is authorized; landing is not yet approved.

2026-10-17 — Jack Heart corrected creation during PR review: “--no-wave should
be default”. Omitted Wave selection creates a standalone repository Task even
inside a Wave/Task-bound conversation or checkout. Only an explicit `--wave`
selects membership; ambient/inherited Wave context must not choose it or block
standalone creation. Keep explicit `--no-wave` as the same standalone choice.
This supersedes the earlier ambient-Wave inheritance in the creation design.
Update creation, its explanation, tests, documentation and PR; return to demo
review without merging. No other review choice is decided by this correction.

Jack Heart also asked whether derived `wave_id` should be a function. The review
clarification: `Task` carries a read projection populated by the Project join,
not a stored Task Wave column; save validates the projected value against Project.
No replacement API was requested. Clarify the field comment without introducing
a second owner or a store lookup disguised as a pure accessor.

Jack Heart accepted retaining the in-memory field provided the database schema
owns the relationship correctly: “just make it clean and clear”. Keep repository
and optional Project as persisted Task identity/membership, derive Wave in the
read projection, and document its save-time consistency validation. This review
question is resolved; no getter or schema redesign is required.

Jack Heart then approved the overall PR review (“ok, overall lg2m”). Complete
the standalone-default correction above, then proceed through the authored ship
Flow and its required checks to landing. This supersedes the earlier landing
hold. Do not describe native hands-on checks as performed; retain their evidence
limits and let the ship gate resolve the required automated acceptance.
