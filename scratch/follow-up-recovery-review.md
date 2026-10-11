# Follow-up conversion: retained effects

2026-10-10 · LOO-412 · Source review supporting Jack Heart's accepted replacement
of follow-through with ordinary Tasks and comments. This narrows the preservation
proof in [audit section 13](work-model-reduction.md#13-finish-every-follow-through-producer-and-consumer);
it adds no product scope or approval requirement.

At `f2ca2063b5`, `sqlite/task_follow_through.rs::create_reserved_task_in` has two
different retained inputs. New local intents already reserve an ordinary Task.
Historical provider-UUID intents can precede any local Task and have **no recorded
network attempt boundary**. `planning_export.rs::retain_follow_through_export_in`
therefore preserves their exact provider creation ID/input as attempted and unknown.
Conversion must retain that uncertainty and recover the same Task; absence of a
local row cannot justify a new creation ID. An `existing` intent must never create.

A second external effect is separate: `ops/planning_export.rs::sync_follow_through_relations`
retries an exact Linear relation ID until confirmation. Retiring new relation
production cannot turn an unconfirmed old relation into a successful comment or
an unattempted effect. Preserve its original input and confirmation history under
concrete historical recovery, without retaining the active FollowThrough model,
completion ritual, UI or generic relationship subsystem for new work.

The conversion proof should cover a reserved local child with missing link, an
uncertain provider creation without a local child, and an unconfirmed relation
after both endpoints are published. Run conversion/retry twice: stable Task and
comment identities, unchanged original events/effect inputs, and no duplicate
creation. New follow-ups use ordinary creation/comment receipts; comment failure
must retry only the comment. These are source-derived counterexamples and required
preservation checks, not executed test results.
