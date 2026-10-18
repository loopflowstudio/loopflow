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
