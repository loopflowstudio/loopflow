# Remaining scope and evidence

October 15, 2026 — LOO-462. Jack Heart accepted Study 06 and requested building and shipping the final scope. Product iteration is closed; only implementation completion, polish, bugs, performance and validation remain.

Accepted: stationary Wave rows and inline Tasks; independent folding; full-size Task-only Available section; no duplicate Task appearances; Ink on Paper; one selected destination; retained nested Work/pane layouts; sidebar resize/collapse; keyboard-only history. `task-workspaces.md` owns the final design and shipping checks. The obsolete drill-in and adjacent-column alternatives are rejected.

The completion investigation measured a roughly 1.2-second command and reproduced indefinite post-command confirmation despite heartbeats. It did not establish the cause or duration of the historical UI stall. The final implementation bounds confirmation and distinguishes its failure from command failure; no native timing target is invented. Diagnostic history stays off the normal Task-frame path.

Separate Tasks retain their scopes: LOO-487 terminal shader/contrast polish, LOO-488 theming, LOO-489 dashboards. For dashboards, Jack clarified a Wave view is strictly query-derived from its conversation and open Tasks; only the pinned repo dashboard is customizable. Default Wave dashboard versus conversation-only remains the child's design question. Tasks have no dashboards. LOO-489 was started in its own checkout/session, stacked at `7d10108eaf`.

Unrelated feedback remains unresolved: Fantasia → manabot could mean folder or display naming; Studio/Silencio portfolio removal did not authorize deleting repository directories. Etude hook failure cause is unknown. These are not silently completed or added to the shipping scope.
