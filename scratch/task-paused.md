# Pause Tasks — lf-2086e0a

Jack Heart authorized this independent extraction and shipping from LOO-462 on
October 13. Source: 331ced510c, excluding scratch and unrelated sidebar policy.

Ship an explicit portable Task planning hold, lf task pause/resume and Desktop
controls using the common writer. Hide paused Tasks from the active sidebar even
with retained Sessions; keep them in the Wave plan. Preserve identity, checkout,
PR, Sessions and Workflow; resume starts nothing. Hold future Task Flow and CI
repair admission; do not stop live processes. Linear disposition is independent.
No new always-visible button, alternate planning store or automatic unpause.

Preserve main's existing non-paused navigation; LOO-462 owns its redesign.
The Paused migration must ship through the ordinary release/install boundary.
Neither real-store schema changes nor closing/replacing the two live demo apps
belongs to this extraction. Jack's LOO-389 pause follows supported deployment.

Remaining: standalone focused Rust/Swift checks, migration and portable exchange,
CLI/wire checks, review, publication and Task-completing landing. CI owns its full
matrix. Native controls and configured demo acceptance remain with LOO-462.

Review kept main's non-paused Session visibility and fixed Task status incorrectly
recommending execution while paused. Existing Workflow/process state remains
independent. The first Swift build was invalidated by an in-flight test edit;
its unchanged-source rerun passes. No native demo or live database mutation.
Check: standalone Rust persistence, exchange, migration, snapshot/actions and DTO tests PASS; documented_commands 3 PASS; Desktop TaskHistory/TaskFlow/paused DTO 18 PASS; fmt, all-target Clippy and diff check PASS. Full matrix: CI.
