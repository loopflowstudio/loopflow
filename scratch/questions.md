# LOO-666 decisions — 2026-10-27

No unresolved product choice blocks implementation. Jack Heart's accepted
requirement for actual finished-work evidence supersedes the existing ordinary
prune age heuristic. Remove it for every prune caller; retain explicit manual
deletion and exact-head closed-PR abandonment semantics. Non-GitHub checkouts
without terminal evidence remain available for explicit disposal.
