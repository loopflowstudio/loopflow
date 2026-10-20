# LOO-533 questions and assumptions

1. **Density build budget unmet (2026-10-20).** After grouping heads, `lf-world build
   density` took 448 s wall / 228 s CPU (was 661 s) with load average ~50 on this Mac.
   The import loop left the profile; the rest is ~44 git subprocesses per Task save:
   `task_create` acquires before creating, `publish_destination` acquires again, and
   `confirm` fetches a third time (8 git calls each), plus save/push. Both acquisitions
   are deliberate ("never publish from a stale local document"), so this pass keeps
   them. Cutting a redundant acquisition is a product decision for its own Task; the
   swift CI step and `world_recipes` now pay the build once each.
2. **Old proof asserted vanished identifiers.** `work-orphans-open`, `session-row-*`,
   `debug-orphan-session-*` and `work-session-count-*` no longer exist in
   `WorkNavigator`, and Task rows appear only for opened started Tasks. The new proof
   opens product's 8 started Tasks, then asserts 8 `work-task-*` rows, one row per
   Wave, and no Session rows for orphans. Tasks are titled from the four phrases plus
   index ("Keep Task observation read-only 7"), not "Work item 7".
